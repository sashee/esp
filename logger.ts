import fs from "node:fs/promises";
import {modifiedCrc, commands} from "./utils.ts";
import { parseArgs } from 'node:util';
import assert from "node:assert/strict";
import {setTimeout} from "node:timers/promises";
import {EventEmitter, once} from "node:events";
import process from "node:process";
import path from "node:path";
import net from "node:net";
import {execFile} from "node:child_process";

// https://r1ch.net/blog/node-v20-aggregateeerror-etimedout-happy-eyeballs
// https://github.com/nodejs/node/issues/54359
//net.setDefaultAutoSelectFamilyAttemptTimeout(1000);

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

const [readFd, writeFd] = await Promise.race([
	setTimeout(5000).then(() => {throw new Error("Open timeout")}),
	Promise.all([
		fs.open(read_from, "r"),
		fs.open(write_to, "w"),
	]),
]).catch((e) => {
	console.error(e);
	// force stop
	process.abort();
});

//await execFile("stty", ["-F", "/dev/ttyUSB0", "sane"]);
//await execFile("stty", ["-F", "/dev/ttyUSB0", "2400", "raw", "-echo"]);

const readResponses = async function*(fd: typeof readFd) {
	let buffer = new Uint8Array(0);
	while(true) {
		const readByte = await fd.read();
		if (readByte.bytesRead === 0) {
			break;
		}
		buffer = new Uint8Array([...buffer, ...readByte.buffer.subarray(0, readByte.bytesRead)]);
		console.log([...buffer].map((a) => a >= 32 && a <= 126 ? String.fromCharCode(a) : "\\x" + a.toString(16).padStart(2, "0")).join(""));

		const checkCrc = (numbers: Uint8Array) => {
			const calculatedCrc = modifiedCrc(new Uint8Array(numbers.slice(0, -2)));
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
							console.log(`CRC not correct. Got: ${[...bytes.subarray(-3, -1)].map((a) => a.toString(16).padStart(2, "0")).join("")}, but expected: ${[...modifiedCrc(new Uint8Array(bytes.slice(0, -3)))].map((a) => a.toString(16).padStart(2, "0")).join("")}. Message: ${[...bytes].map((a) => a.toString(16).padStart(2, "0")).join("")}`);
							return [];
						}
						console.log(`CRC correct. Got: ${[...bytes.subarray(-3, -1)].map((a) => a.toString(16).padStart(2, "0")).join("")}. Message: ${[...bytes].map((a) => a.toString(16).padStart(2, "0")).join("")}`);
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
			console.log("Got response to command: " + foundResponse.command);
			yield {command: foundResponse.command, values: foundResponse.values};
		}
	}
}

const sendCommand = (write: typeof writeFd, commandEvents: EventEmitter) => async (command: string) => {
	const commandBytes = new TextEncoder().encode(command);
	const fullCommandBytes = new Uint8Array([...commandBytes, ...modifiedCrc(commandBytes), 13]);
	console.log("writing", command, fullCommandBytes);

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

const readCredential = async (credentialName: string, envVarName: string) => {
	if (process.env.CREDENTIALS_DIRECTORY) {
		return await fs.readFile(path.join(process.env.CREDENTIALS_DIRECTORY, credentialName), "utf8");
	}else {
		return process.env[envVarName];
	}
}

const thingspeakKey = await readCredential("thingspeak-key", "THINGSPEAK_KEY");

await Promise.all([
	(async () => {
		for await (const comm of commandsGen) {
			commandEvents.emit(comm.command, comm.values);
		}
	})(),
	(async () => {
		while (true) {
			const qpigs = await sender("QPIGS");
			console.log(qpigs);
			const qpigs2 = await sender("QPIGS2");
			console.log(qpigs2);
			const qpiws = await sender("QPIWS");
			console.log(qpiws);

			if (qpigs.battery_charging_current !== 0 && qpigs.battery_discharge_current !== 0) {
				throw new Error(`Both battery charging current and battery discharge current are non-null! qpigs.battery_charging_current = ${qpigs.battery_charging_current}, qpigs.battery_discharge_current = ${qpigs.battery_discharge_current}`);
			}

			console.log(`INVERTER_DATA_LOGGING ${JSON.stringify({qpigs, qpigs2, qpiws, time: new Date().toISOString(), uptime: new Date().getTime() - startTime})}`);
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
				"?api_key=" + thingspeakKey + Object.entries(fields).map(([k, v]) => `&${k}=${v}`).join(""));
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

