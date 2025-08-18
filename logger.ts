import fs from "node:fs/promises";
import {modifiedCrc, commands, bms} from "./utils.ts";
import {setTimeout} from "node:timers/promises";
import process from "node:process";
import path from "node:path";
import net from "node:net";
import { autoDetect } from "@serialport/bindings-cpp";
import { Buffer } from 'node:buffer';
import {DatabaseSync} from "node:sqlite";

const database = new DatabaseSync(path.join(process.env.HOME, "monitoring-data", "data.db"));

database.exec(`
  CREATE TABLE IF NOT EXISTS data(
    timestamp INTEGER PRIMARY KEY,
    value TEXT
  ) STRICT
`);

const insertIntoDb = database.prepare('INSERT INTO data (timestamp, value) VALUES (?, ?)');

// https://r1ch.net/blog/node-v20-aggregateeerror-etimedout-happy-eyeballs
// https://github.com/nodejs/node/issues/54359
//net.setDefaultAutoSelectFamilyAttemptTimeout(1000);

const binding = autoDetect();

const request = async (path: string, command: typeof commands[0], signal: AbortSignal) => {
	const port = await binding.open({path, baudRate: 2400, dataBits: 8, stopBits: 1, parity: "none"});
	const finished = new AbortController();
	try {
		signal.throwIfAborted();
		const [, , values] = await Promise.all([
			new Promise((res, rej) => {
				signal.addEventListener("abort", (reason) => {
					port.close().catch((e) => rej(e));
					rej(reason);
				}, {once: true, signal: finished.signal});
				finished.signal.addEventListener("abort", (r) => res(r), {once: true, signal});
			}),
			(async () => {
				const commandBytes = new TextEncoder().encode(command.command);
				const fullCommandBytes = new Uint8Array([...commandBytes, ...modifiedCrc(commandBytes), 13]);
				try {
					await port.write(Buffer.from(fullCommandBytes, fullCommandBytes.byteLength, fullCommandBytes.byteLength));
				}catch(e) {
					if (!e.canceled) {
						throw e;
					}
				}
			})(),
			(async () => {
				let buffer = new Uint8Array(0);
				while(true) {
					const readBytes = await (async () => {
						try {
							const {buffer, bytesRead} = await port.read(Buffer.alloc(8), 0, 8);
							return buffer.subarray(0, bytesRead);
						}catch(e) {
							if (!e.canceled) {
								throw e;
							}
						}
					})();
					if (readBytes === undefined) {
						break;
					}
					buffer = new Uint8Array([...buffer, ...readBytes]);
					//console.log([...buffer].map((a) => a >= 32 && a <= 126 ? String.fromCharCode(a) : "\\x" + a.toString(16).padStart(2, "0")).join(""));

					const checkCrc = (numbers: Uint8Array) => {
						const calculatedCrc = modifiedCrc(new Uint8Array(numbers.slice(0, -2)));
						const actualCrc = numbers.slice(-2);
						return calculatedCrc.length === actualCrc.length && calculatedCrc.every((v, i) => v === actualCrc[i]);
					};

					const {skipTo, result} = buffer.reduce(({skipTo, result}, e, i) => {
						if (i < skipTo || result !== undefined) {
							return {skipTo, result};
						}else {
							if (e === 40) {
								if (buffer[i + command.length + 3] !== 13) {
									return [];
								}
								const bytes = new Uint8Array(buffer.slice(i, i + command.length + 4));
								const crcOk = checkCrc(bytes.subarray(0, -1));
								if (!crcOk) {
									console.log(`CRC not correct. Got: ${[...bytes.subarray(-3, -1)].map((a) => a.toString(16).padStart(2, "0")).join("")}, but expected: ${[...modifiedCrc(new Uint8Array(bytes.slice(0, -3)))].map((a) => a.toString(16).padStart(2, "0")).join("")}. Message: ${[...bytes].map((a) => a.toString(16).padStart(2, "0")).join("")}`);
									return [];
								}
								//console.log(`CRC correct. Got: ${[...bytes.subarray(-3, -1)].map((a) => a.toString(16).padStart(2, "0")).join("")}. Message: ${[...bytes].map((a) => a.toString(16).padStart(2, "0")).join("")}`);
								const parsed = command.parse(new TextDecoder().decode(bytes.subarray(1, -3)));
								if (!parsed) {
								return {skipTo, result};
								}else {
									return {
										skipTo: i + command.length + 3,
										result: {
											from: i,
											to: i + command.length + 3,
											command: command.command,
											values: parsed,
											bytes,
										}
									};
								}
							}else {
								return {skipTo, result};
							}
						}
					}, {skipTo: 0, result: undefined});

					buffer = new Uint8Array(buffer.subarray(skipTo));

					if (result) {
						finished.abort();
						return result.values;
					}
				}
			})(),
		]);
		return values;
	}finally {
		if (port.isOpen) {
			await port.close();
		}
	}
};

const readBms = async (path: string, signal: AbortSignal) => {
	const port = await binding.open({path, baudRate: 115200, dataBits: 8, stopBits: 1, parity: "none"});
	const finished = new AbortController();
	try {
		signal.throwIfAborted();
		const [, values] = await Promise.all([
			new Promise((res, rej) => {
				signal.addEventListener("abort", (reason) => {
					port.close().catch((e) => rej(e));
					rej(reason);
				}, {once: true, signal: finished.signal});
				finished.signal.addEventListener("abort", (r) => res(r), {once: true, signal});
			}),
			(async () => {
				let buffer = new Uint8Array(0);
				while(true) {
					const readBytes = await (async () => {
						try {
							const {buffer, bytesRead} = await port.read(Buffer.alloc(560), 0, 560);
							return buffer.subarray(0, bytesRead);
						}catch(e) {
							if (!e.canceled) {
								throw e;
							}
						}
					})();
					if (readBytes === undefined) {
						break;
					}
					buffer = new Uint8Array([...buffer, ...readBytes]);
					//console.log([...buffer].map((a) => a.toString(16).padStart(2, "0")).join(""));

					const {skipTo, result} = buffer.reduce(({skipTo, result}, e, i) => {
						if (i < skipTo || result !== undefined) {
							return {skipTo, result};
						}else {
							if (e === 85) { // hex 55
								const bytes = new Uint8Array(buffer.slice(i, i + bms.length));
								const parsed = bms.parse(bytes);
								if (!parsed) {
									return {skipTo, result};
								}else {
									return {
										skipTo: i + bms.length + 1,
										result: {
											from: i,
											to: i + bms.length,
											values: parsed,
											bytes,
										}
									};
								}
							}else {
								return {skipTo, result};
							}
						}
					}, {skipTo: 0, result: undefined});
					buffer = new Uint8Array(buffer.subarray(skipTo));

					if (result) {
						finished.abort();
						return result.values;
					}
				}
			})(),
		]);
		return values;
	}finally {
		if (port.isOpen) {
			await port.close();
		}
	}
};

const getAllUSBPaths = async () => {
	const serialList = await binding.list();
	console.log(serialList)
	return serialList.map(({path}) => path).filter((path) => path.includes("USB"));
};

const detectBmsPath = async (paths: string[]) => {
	console.log(`Trying to find bms path: ${paths}`);
	const results = await Promise.allSettled(paths.map(async (path) => {
		await readBms(path, AbortSignal.timeout(10000));
		return path;
	}));
	console.log(results);
	const foundPath = results.filter(({status}) => status === "fulfilled").map(({value}) => value);
	if (foundPath.length === 0) {
		throw new Error("Could not find the path for the bms");
	}
	return foundPath[0];

}

const detectInverterPath = async (paths: string[]) => {
	console.log(`Trying to find inverter path: ${paths}`);
	const results = await Promise.allSettled(paths.map(async (path) => {
		await request(path, commands.find(({command}) => command === "QPIGS")!, AbortSignal.timeout(2000));
		return path;
	}));
	console.log(results);
	const foundPath = results.filter(({status}) => status === "fulfilled").map(({value}) => value);
	if (foundPath.length === 0) {
		throw new Error("Could not find the inverter path");
	}
	return foundPath[0];
}

const allUSBPaths = await getAllUSBPaths();
const bmsPath = await detectBmsPath(allUSBPaths);

const inverterPath = await detectInverterPath(allUSBPaths.filter((p) => p !== bmsPath));

console.log(`Inverter path: ${inverterPath}`);

const startTime = new Date().getTime();

const readCredential = async (credentialName: string, envVarName: string) => {
	if (process.env.CREDENTIALS_DIRECTORY) {
		return await fs.readFile(path.join(process.env.CREDENTIALS_DIRECTORY, credentialName), "utf8");
	}else {
		return process.env[envVarName];
	}
}

const thingspeakKey = await readCredential("thingspeak-key", "THINGSPEAK_KEY");

while (true) {
	const [{qpigs, qpigs2, qpiws}, bms] = await Promise.all([
		(async () => {
			const qpigs = await request(inverterPath, commands.find(({command}) => command === "QPIGS")!, AbortSignal.timeout(2000));
			console.log(qpigs);
			const qpigs2 = await request(inverterPath, commands.find(({command}) => command === "QPIGS2")!, AbortSignal.timeout(2000));
			console.log(qpigs2);
			const qpiws = await request(inverterPath, commands.find(({command}) => command === "QPIWS")!, AbortSignal.timeout(2000));
			console.log(qpiws);
			return {qpigs, qpigs2, qpiws};
		})(),
		(async () => {
			return await readBms(bmsPath, AbortSignal.timeout(10000));
		})(),
	]);
	console.log(bms);

	if (qpigs.battery_charging_current !== 0 && qpigs.battery_discharge_current !== 0) {
		throw new Error(`Both battery charging current and battery discharge current are non-null! qpigs.battery_charging_current = ${qpigs.battery_charging_current}, qpigs.battery_discharge_current = ${qpigs.battery_discharge_current}`);
	}

	insertIntoDb.run(new Date().getTime(), JSON.stringify({inverter: {qpigs, qpigs2, qpiws}, battery: {bms}}));

	const fields = {
		field1: (new Date().getTime() - startTime) / 1000,
		field2: qpigs.ac_output_active_power,
		field3: qpigs.battery_voltage,
		field4: qpigs.battery_charging_current - qpigs.battery_discharge_current,
		field5: bms.state_of_charge,
		field6: bms.cycle_count,
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

	await setTimeout(1000 * 60 * 5);
}

