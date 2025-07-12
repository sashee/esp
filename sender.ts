import fs from "node:fs/promises";
import {crc_xmodem, processLogs, commands} from "./utils.ts";
import { parseArgs } from 'node:util';
import assert from "node:assert/strict";

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

/*
await execFile("stty", ["-F", "/dev/ttyUSB0", "2400"]);
await execFile("stty", ["-F", "/dev/ttyUSB0", "raw"]);
*/

const writeFd = await fs.open(write_to, "w");
const readFd = await fs.open(read_from, "r");

const readCommands = async function*(fd: typeof readFd) {
	let buffer = new Uint8Array(0);
	while(true) {
		const readByte = await fd.read();
		console.log(readByte);
		if (readByte.bytesRead === 0) {
			break;
		}
		buffer = new Uint8Array([...buffer, ...readByte.buffer.subarray(0, readByte.bytesRead)]);
		const foundCommands = commands.map(({command}) => command).flatMap((command) => {
				const commandBytes = new TextEncoder().encode(command);
				const fullCommandBytes = new Uint8Array([...commandBytes, ...crc_xmodem(commandBytes), 13]);
				const foundCommandIdx = buffer.findLastIndex((e, i, l) => {
					return fullCommandBytes.every((ce, ci) => l[i + ci] === ce);
				});
				if (foundCommandIdx > -1) {
					return [{from: foundCommandIdx, to: foundCommandIdx + fullCommandBytes.length, command}];
				}else {
					return [];
				}
		}).toSorted((a, b) => a.to - b.to);

		for (const foundCommand of foundCommands) {
			yield foundCommand.command;
		}
		buffer = new Uint8Array(buffer.subarray((foundCommands.at(-1)?.to ?? -1) + 1));
	}
}

const contents: string = await fs.readFile("logs.txt", "utf8");
const {messages} = processLogs(contents);
const messageGroupForCommands = Object.groupBy(messages, ({command}) => command);

const commandsGen = readCommands(readFd);
const messageCounters = {};
for await (const comm of commandsGen) {
	if (messageGroupForCommands[comm]) {
		// random
		//const messageIndex = ((messageCounters[comm] ?? 0) + Math.floor(Math.random() * messageGroupForCommands[comm].length)) % messageGroupForCommands[comm].length;
		// time-ordered
		const messageIndex = ((messageCounters[comm] ?? 0) + 1) % messageGroupForCommands[comm].length;
		messageCounters[comm] = messageIndex;

		console.log(comm);
		const randomBytes = (num: number) => Array(Math.floor(Math.random() * num)).fill(undefined).map(() => {
			if (Math.random() < 0.1) {
				return Math.random() < 0.5 ? 40 : 13;
			}else {
				return Math.floor(Math.random() * 255)
			}
		});
		const messageBytes = new Uint8Array(messageGroupForCommands[comm][messageIndex].bytes);
		await writeFd.write(new Uint8Array([...randomBytes(1000), ...messageBytes, ...randomBytes(50)]));
	}else {
		console.warn(`Unknown command: ${comm}`);
	}
}
