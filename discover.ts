import process from "node:process";
import fs from "node:fs/promises";
import {execFile} from "node:child_process";
import {setTimeout} from "node:timers/promises";
import { once, EventEmitter } from 'node:events';
import { parseArgs } from 'node:util';

const {baud, message} = parseArgs({options: {
  baud: {
    type: 'string',
  },
  message: {
    type: 'string',
  },
}}).values;

const bauds = [1200, 1800, 2400, 4800, 9600, 19200, 28800, 38400, 57600, 76800, 115200];
const dev = "/dev/ttyUSB1";
const messages = ["4E5700130000000006030000000000006800000129"];

const fromHexString = (hexString) =>
  Uint8Array.from(hexString.match(/.{1,2}/g).map((byte) => parseInt(byte, 16)));

const tryCombination = async (baud: number, message: Uint8Array) => {
	console.log(`Trying ${baud} ${message}`);
	await execFile("stty", ["-F", dev, "sane"]);
	await execFile("stty", ["-F", dev, baud, "raw", "-echo"]);

	const fd = await fs.open(dev, "r+");
	try {
		const signal = AbortSignal.timeout(2000);
		await Promise.race([
			(async () => {
				const v = await fd.read();
				console.log("======================================================================<<<<<<<<<<<<<<<<<<<<<<<<<<<");
				console.log("GOT SOME MESSAGES");
				console.log(v);
			})(),
			(async () => {
				await fd.write(message);
				await once(signal, "abort");
			})(),
		]);
	}catch(e) {
		console.error(e);
	}finally {
		process.abort(0);
	}
}

await tryCombination(Number(baud), fromHexString(message));
