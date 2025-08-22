import fs from "node:fs/promises";
import plot from 'simple-ascii-chart';
import {bms, hexToArr} from "../utils.ts";

const msg = "55aaeb900200e10ce30ce30ce70ce50ce30ce20ce60ce50ce60ce50ce60ce60ce70ce70ce50c0000000000000000000000000000000000000000000000000000000000000000ffff0000e50c06000d00440042004500420045004200450042004600440045004300460044004600440000000000000000000000000000000000000000000000000000000000000000003c01000000004ace0000c8c30600cb20000033013101000008000000003f32390200708203003e0000009ec3dc0064000000c981b00001010000000000000000000000000000ff0001000000b6031600000044303e4000000000a1140000000101010006010026b00900000000003c0136013701ba03f3b1950af50200008051010000000301000000000000000000feff7fdc2f0101b00f000000a7";

console.log(msg.length)

console.log(bms.parse(hexToArr(msg)));
/*
const matched = msgs.match(pattern).groups;
const values = (parse(matched));
Object.entries(values).map(([k, v]) => {
	if (typeof v === "number") {
console.log(plot(
	[...msgs.match(new RegExp(pattern, "g"))].map((str, i) => {
		const matched = str.match(pattern).groups;
		return [i, parse(matched)[k]]
	}),
  { width: 150, height: 28, legend: { position: 'top', series: [k] },},
));

	}
})
*/
