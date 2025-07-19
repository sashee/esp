import * as readline from 'node:readline/promises';
import { stdin as input, stdout as output } from 'node:process';
import {execFile} from "node:child_process";
import {setTimeout} from "node:timers/promises";
import fs from "node:fs/promises";
import process from "node:process";
import {modifiedCrc} from "./utils.ts";

await execFile("stty", ["-F", "/dev/ttyUSB0", "sane"]);
await execFile("stty", ["-F", "/dev/ttyUSB0", "2400", "raw", "-echo"]);

const read_from = "/dev/ttyUSB0";
const write_to = "/dev/ttyUSB0";

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

const rl = readline.createInterface({ input, output });

while (true) {
	const command = await rl.question("> ");

	console.log(command);

	const commandBytes = new TextEncoder().encode(command);
	const fullCommandBytes = new Uint8Array([...commandBytes, ...modifiedCrc(commandBytes), 13]);
	console.log([...fullCommandBytes].map((n) => n.toString(16).padStart(2, "0")))
	writeFd.write(fullCommandBytes);
}

rl.close();
