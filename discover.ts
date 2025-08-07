import {setTimeout} from "node:timers/promises";
import process from "node:process";
import { once, EventEmitter } from 'node:events';
import { LinuxBinding } from "@serialport/bindings-cpp";
import { Buffer } from 'node:buffer';

const optionParts: {
	path: Array<Parameters<(typeof LinuxBinding)["open"]>[0]["path"]>,
	baudRate: Array<Parameters<(typeof LinuxBinding)["open"]>[0]["baudRate"]>,
	dataBits: Array<Parameters<(typeof LinuxBinding)["open"]>[0]["dataBits"]>,
	stopBits: Array<Parameters<(typeof LinuxBinding)["open"]>[0]["stopBits"]>,
	parity: Array<Parameters<(typeof LinuxBinding)["open"]>[0]["parity"]>,
} = {
	path: ["/dev/ttyUSB1"],
	baudRate: [115200, 1200, 1800, 2400, 4800, 9600, 19200, 28800, 38400, 57600, 76800],
	dataBits: [8, 5,6,7],
	stopBits: [1,1.5,2],
	parity: ["none", "even", "odd"],
};

const messages = ["4E5700130000000006030000000000006800000129", "AA5590EB96000000000000000000000000000010", "AA5590EB97000000000000000000000000000011"];

const fromHexString = (hexString) =>
  Uint8Array.from(hexString.match(/.{1,2}/g).map((byte) => parseInt(byte, 16)));

const request = async (options: Parameters<(typeof LinuxBinding)["open"]>[0], bytes: Uint8Array, signal: AbortSignal) => {
	const port = await LinuxBinding.open(options);
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
				try {
					await port.write(Buffer.from(bytes, bytes.byteLength, bytes.byteLength));
				}catch(e) {
					if (!e.canceled) {
						throw e;
					}
				}
			})(),
			(async () => {
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
					console.log(readBytes);
					finished.abort();
					return readBytes;
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

for (const path of optionParts.path) {
	for (const baudRate of optionParts.baudRate) {
		for (const dataBits of optionParts.dataBits) {
			for (const stopBits of optionParts.stopBits) {
				for (const parity of optionParts.parity) {
					for (const message of messages) {
						const options = {path, baudRate, dataBits, stopBits, parity};
						console.log(`Trying ${JSON.stringify(options)} with ${message}`);
						await request(options, fromHexString(message), AbortSignal.timeout(2000))
						.then(() => {
							console.log("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
							process.exit(1);
						})
						.catch((e) => {console.log("Nope", e)});
					}
				}
			}
		}
	}
}
