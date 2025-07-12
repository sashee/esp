import fs from "node:fs/promises";
import {crc_xmodem, processLogs, commands} from "./utils.ts";
import { parseArgs } from 'node:util';
import assert from "node:assert/strict";
import {setTimeout} from "node:timers/promises";
import {EventEmitter, once} from "node:events";
import process from "node:process";

const {read_from, write_to} = parseArgs({options: {
  read_from: {
    type: 'string',
  },
  write_to: {
    type: 'string',
  },
}}).values;
assert(read_from);
assert(write_to);

const readFd = await fs.open(read_from, "r");
const writeFd = await fs.open(write_to, "w");

const readResponses = async function*(fd: typeof readFd) {
	let buffer = new Uint8Array(0);
	while(true) {
		const readByte = await fd.read();
		if (readByte.bytesRead === 0) {
			break;
		}
		buffer = new Uint8Array([...buffer, ...readByte.buffer.subarray(0, readByte.bytesRead)]);

		const checkCrc = (numbers: Uint8Array) => {
			const calculatedCrc = crc_xmodem(new Uint8Array(numbers.slice(0, -2)));
			const actualCrc = numbers.slice(-2);
			return calculatedCrc.length === actualCrc.length && calculatedCrc.every((v, i) => v === actualCrc[i]);
		};

		const {skipTo, results} = buffer.reduce(({skipTo, results}, e, i) => {
			if (i < skipTo) {
				return {skipTo, results};
			}else {
				if (e === 40) {
					const matchedCommands = commands.flatMap((command) => {
						if (buffer[i + command.length + 3] !== 13) {
							return [];
						}
						const bytes = new Uint8Array(buffer.slice(i, i + command.length + 4));
						const crcOk = checkCrc(bytes.subarray(0, -1));
						if (!crcOk) {
							return [];
						}
						const parsed = command.parse(new TextDecoder().decode(bytes.subarray(1, -3)));
						if (!parsed) {
							return [];
						}else {
							return [{
								from: i,
								to: i + command.length + 3,
								command: command.command,
								values: parsed,
								bytes,
							}];
						}
					});
					if (matchedCommands.length > 0) {
						return {
							skipTo: matchedCommands[0].to + 1,
							results: [...results, matchedCommands[0]],
						};
					}else {
						return {skipTo, results};
					}
				}else {
					return {skipTo, results};
				}
			}
		}, {skipTo: 0, results: []});

		buffer = new Uint8Array(buffer.subarray(skipTo));

		for (const foundResponse of results) {
			yield {command: foundResponse.command, values: foundResponse.values};
		}
	}
}

const sendCommand = (write: typeof writeFd, commandEvents: EventEmitter) => async (command: string) => {
	const commandBytes = new TextEncoder().encode(command);
	const fullCommandBytes = new Uint8Array([...commandBytes, ...crc_xmodem(commandBytes), 13]);

	const [[res]] = await Promise.all([
		once(commandEvents, command, {signal: AbortSignal.timeout(5000)}),
		write.write(fullCommandBytes),
	]);
	return res;
}

const commandEvents = new EventEmitter();
const commandsGen = readResponses(readFd);
const sender = sendCommand(writeFd, commandEvents);

const startTime = new Date().getTime();

await Promise.all([
	(async () => {
		for await (const comm of commandsGen) {
			commandEvents.emit(comm.command, comm.values);
		}
	})(),
	(async () => {
		while (true) {
			const qpigs = await sender("QPIGS");
			const qpigs2 = await sender("QPIGS2");
			const qpiws = await sender("QPIWS");
			console.log(qpigs);
			console.log(qpigs2);
			console.log(qpiws);

			if (qpigs.battery_charging_current != 0 && qpigs.battery_discharge_current != 0) {
				throw new Error(`Both battery charging current and battery discharge current are non-null! qpigs.battery_charging_current = ${qpigs.battery_charging_current}, qpigs.battery_discharge_current = ${qpigs.battery_discharge_current}`);
			}
			const fields = {
				field1: (new Date().getTime() - startTime) / 1000,
				field2: qpigs.ac_output_active_power,
				field3: qpigs.battery_voltage,
				field4: qpigs.battery_charging_current - qpigs.battery_discharge_current,
				field5: qpigs.battery_capacity,
				field6: qpigs.inverter_heat_sink_temperature,
				field7: qpigs.pv_charging_power1,
				field8: qpigs2.pv_charging_power2,
			};
			console.log(fields);
			const req = await fetch("https://api.thingspeak.com/update" + 
				"?api_key=" + process.env.THINGSPEAK_KEY + Object.entries(fields).map(([k, v]) => `&${k}=${v}`).join(""));
			if (!req.ok) {
				console.error(req);
				throw new Error("Could not upload to thingspeak");
			}

			await setTimeout(15000);
		}
	})(),
]).catch((e) => {
	console.error(e);
	// force stop
	process.abort();
});

