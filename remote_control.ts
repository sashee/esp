import {setTimeout} from "node:timers/promises";
import {spawn} from "node:child_process";
import process from "node:process";
import path from "node:path";
import { Buffer } from 'node:buffer';
import fs from "node:fs/promises";

const readCredential = async (credentialName: string, envVarName: string) => {
	if (process.env.CREDENTIALS_DIRECTORY) {
		return await fs.readFile(path.join(process.env.CREDENTIALS_DIRECTORY, credentialName), "utf8");
	}else {
		return process.env[envVarName];
	}
}

const remoteControlToken = await readCredential("remote-control-token", "TOKEN");
const remoteControlChatId = Number(await readCredential("remote-control-chat_id", "CHAT_ID"));

const sendTelegramCommand = (token: string) => async (command: string, params: object) => {
	const res = await fetch(`https://api.telegram.org/bot${token}/${command}`, {
		method: "POST",
			headers: {
			"Content-Type": "application/json",
		},
		body: JSON.stringify(params),
	});
	if (!res.ok) {
		console.error(res);
		throw new Error("Error in res");
	}
	return await res.json();
}

const sendCommand = sendTelegramCommand(remoteControlToken);

const readMessages = async function*(token: string, chatId: number) {
	let offset: number | undefined = undefined;
	while(true) {
		console.log("getUpdates with offset: " + offset);
		const resJson = await sendCommand("getUpdates", {offset, timeout: 60});
		console.log(JSON.stringify(resJson, undefined, 4));
		offset = Math.max((offset ?? 0), ...resJson.result.map(({update_id}) => update_id)) + 1;
		const messages = resJson.result
			.flatMap((res) => res.message !== undefined && res.message.chat.id === chatId && (new Date(res.message.date * 1000).getTime() > new Date().getTime() - 10*60*1000) ? [res.message] : [])
			.toSorted((m1, m2) => m1.date - m2.date);

			console.log("GOT MESSAGES: ", messages);
		for (const message of messages) {
			yield message;
		}
		await setTimeout(1000);
	}
}

await sendCommand("setMyCommands", {
	commands: [
		{
			command: "pinggy",
			description: "ssh -p 443 -R0:localhost:22 tcp@a.pinggy.io",
		},
	],
});

const messagesGen = readMessages(remoteControlToken, remoteControlChatId);

for await (const message of messagesGen) {
	console.log("goT MESSAge", message);
	if (message.text === "/pinggy") {
		console.log("pinggy!")
		const child = spawn("ssh", ["-o", "StrictHostKeyChecking=no", "-p", "443", "-R0:localhost:22", "tcp@a.pinggy.io"], {env: {PATH: process.env.PATH}});
		const stdout = [];
		const stderr = [];
		let closed = false;
		child.stdout.on("data", (data) => {
			console.log(data.toString());
			stdout.push(data);
		});
		child.stderr.on("data", (data) => {
			console.log("stderr", data.toString());
			stderr.push(data);
		});
		await Promise.all([
			setTimeout(5000).then(async () => {
				if (!closed) {
					const msg = await sendCommand("sendMessage", {
						chat_id: remoteControlChatId,
						text: Buffer.concat(stdout).toString() + "\nstderr:\n" + Buffer.concat(stderr).toString(),
						link_preview_options: {
							is_disabled: true,
						}
					});
					console.log(JSON.stringify(msg, undefined, 4));
				}
			}),
			new Promise((res, rej) => {
				child.on("close", async (code) => {
					console.log("command closed");
					closed = true;
					res(code);
				});
			}),
		]);
		const msg = await sendCommand("sendMessage", {
			chat_id: remoteControlChatId,
			text: "closed",
			link_preview_options: {
				is_disabled: true,
			}
		});
		console.log(JSON.stringify(msg, undefined, 4));
	}
}

