import {setTimeout} from "node:timers/promises";
import process from "node:process";
import { once, EventEmitter } from 'node:events';
import { LinuxBinding } from "@serialport/bindings-cpp";
import { Buffer } from 'node:buffer';
import {bms} from "./utils.ts";

const readBms = async (options: Parameters<(typeof LinuxBinding)["open"]>[0], signal: AbortSignal) => {
	const port = await LinuxBinding.open(options);
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
					console.log([...buffer].map((a) => a.toString(16).padStart(2, "0")).join(""));

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

const res = await readBms({path: "/dev/ttyUSB0", baudRate: 115200, dataBits: 8, stopBits: 1, parity: "none"}, AbortSignal.timeout(3000))
console.log(res);
