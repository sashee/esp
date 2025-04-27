import {execFile} from "node:child_process";
import fs from "node:fs/promises";
import {crc_xmodem, processLogs, commands} from "./utils.ts";

await execFile("stty", ["-F", "/dev/ttyUSB0", "2400"]);
await execFile("stty", ["-F", "/dev/ttyUSB0", "raw"]);

const usbFd = await fs.open("/dev/ttyUSB0", "r+");

const readCommands = async function*(fd) {
	let buffer = new Uint8Array(0);
	while(true) {
		const readByte = await usbFd.read();
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
const messageGroupForCommands = Object.groupBy(messages, ({parsed}) => parsed?.command);

const commandsGen = readCommands(usbFd);
const messageCounters = {};
for await (const comm of commandsGen) {
	if (messageGroupForCommands[comm]) {
		const messageIndex = ((messageCounters[comm] ?? 0) + 1) % messageGroupForCommands[comm].length;
		messageCounters[comm] = messageIndex;

		console.log(comm);
		const randomBytesBefore = Array(Math.floor(Math.random() * 100)).fill(undefined).map(() => Math.floor(Math.random() * 255));
		const randomBytesAfter = Array(Math.floor(Math.random() * 100)).fill(undefined).map(() => Math.floor(Math.random() * 255));
		const messageBytes = new Uint8Array([40, ...new TextEncoder().encode(messageGroupForCommands[comm][messageIndex].message)]);
		await usbFd.write(new Uint8Array([...randomBytesBefore, ...messageBytes, ...crc_xmodem(messageBytes), 13, ...randomBytesAfter]));
	}else {
		console.warn(`Unknown command: ${comm}`);
	}
}
