import fs from "node:fs/promises";
import {processLogs} from "./utils.ts";

const contents: string = await fs.readFile("logs.txt", "utf8");
const {invalidCrcGroups, unparseableMessages, messages} = processLogs(contents);

if (invalidCrcGroups.length > 0) {
	console.warn(`Dropping ${invalidCrcGroups.length} item(s) because of crc match failure`);
}

if (unparseableMessages.length > 0) {
	console.warn(`Could not parse ${unparseableMessages.length} message(s)`);
	console.log(unparseableMessages);
}

console.log(`Total number of messages: ${messages.length}`);

const arrToCsv = (arr: any[]) => {
	const keys = Object.keys(arr[0]);
	return [
		keys.join(","),
		...arr.map((o) => keys.map((key) => o[key]).join(",")),
	].join("\n");
}

Object.entries(Object.groupBy(messages, ({parsed}) => parsed?.command)).map(async ([messageCommand, messagesForThisCommand]) => {
	const writeValues = arrToCsv(messagesForThisCommand.map(({parsed}) => parsed.values));
	await fs.writeFile(`${messageCommand}.csv`, writeValues, "utf8")
});
