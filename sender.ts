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

const commandsGen = readCommands(usbFd);
const messageCounters = {};
for await (const comm of commandsGen) {
	if (messageGroupForCommands[comm]) {
		const messageIndex = ((messageCounters[comm] ?? 0) + Math.floor(Math.random() * messageGroupForCommands[comm].length)) % messageGroupForCommands[comm].length;
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
		await usbFd.write(new Uint8Array([...randomBytes(1000), ...messageBytes, ...randomBytes(50)]));
	}else {
		console.warn(`Unknown command: ${comm}`);
	}
}
