import {setTimeout} from "node:timers/promises";
import process from "node:process";
import { once, EventEmitter } from 'node:events';
import { LinuxBinding } from "@serialport/bindings-cpp";
import { Buffer } from 'node:buffer';

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
					//await port.write(Buffer.from(bytes, bytes.byteLength, bytes.byteLength));
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
					console.log([...readBytes].map((a) => a.toString(16).padStart(2, "0")).join(""));
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

const fromHexString = (hexString) =>
  Uint8Array.from(hexString.match(/.{1,2}/g).map((byte) => parseInt(byte, 16)));

await request({path: "/dev/ttyUSB0", baudRate: 115200, dataBits: 8, stopBits: 1, parity: "none"}, fromHexString("4E5700130000000006030000000000006800000129"), AbortSignal.timeout(2000000000))
