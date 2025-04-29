import fs from "node:fs/promises";
import {processLogs} from "./utils.ts";

const contents: string = await fs.readFile("logs.txt", "utf8");
const {messages, length} = processLogs(contents);

{
	// https://stackoverflow.com/a/47785639
	const width = Math.floor(length ** 0.5);
	const height = Math.ceil(length / width);
	const padding = (4 - (width * 3) % 4) % 4;
	const fileHeaderSize = 14;
	const infoHeaderSize = 40;
	const fileSize = fileHeaderSize + infoHeaderSize + (width * 3 + padding) * height;
	const fileHeader = new Uint8Array([
		66, // B
		77, // M
		fileSize & 0xff, (fileSize >> 8) & 0xff, (fileSize >> 16) & 0xff, (fileSize >> 24) & 0xff, // file size in bytes
		0, 0, 0, 0, // reserved
		fileHeaderSize + infoHeaderSize, 0, 0, 0, // start of pixel array
	]);
	const infoHeader = new Uint8Array([
		infoHeaderSize, 0, 0, 0, // header size
		width & 0xff, (width >> 8) & 0xff, (width >> 16) & 0xff, (width >> 24) & 0xff, // width
		height & 0xff, (height >> 8) & 0xff, (height >> 16) & 0xff, (height >> 24) & 0xff, // height
		1, 0, // number of color planes
		24, 0, // bits per pixel
		0,0,0,0, // compression
		0,0,0,0, // image size
		0,0,0,0, // horizontal resolution
		0,0,0,0, // vertical resolution
		0,0,0,0, // colors in color table
		0,0,0,0, // important color count
	]);

	const bmp = await fs.open("res.bmp", "w");
	await bmp.write(fileHeader);
	await bmp.write(infoHeader);
	const isProcessed = (index: number) => {
		return messages.some(({from, to}) => index >= from && index <= to);
	}
	for (let i = height - 1; i >= 0; i--) {
		await bmp.write(new Uint8Array(Array(width).fill(undefined).flatMap((_e, idx) => (i * height + idx > length) ? [0, 0, 0] : isProcessed(i * height + idx) ? [255, 255, 255] : [0, 0, 255])));
		await bmp.write(new Uint8Array(padding));
	}
	await bmp.sync();
	await bmp.close();
}

console.log(`Total number of messages: ${messages.length}`);

const arrToCsv = (arr: any[]) => {
	const keys = Object.keys(arr[0]);
	return [
		keys.join(","),
		...arr.map((o) => keys.map((key) => o[key]).join(",")),
	].join("\n");
}

Object.entries(Object.groupBy(messages, ({command}) => command)).map(async ([messageCommand, messagesForThisCommand]) => {
	const writeValues = arrToCsv(messagesForThisCommand.map(({values}) => values));
	await fs.writeFile(`${messageCommand}.csv`, writeValues, "utf8")
});
