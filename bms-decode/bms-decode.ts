import fs from "node:fs/promises";
import plot from 'simple-ascii-chart';

const msgs = await fs.readFile("bms.txt", "utf8");
const msg = "55aaeb900200e10ce30ce30ce70ce50ce30ce20ce60ce50ce60ce50ce60ce60ce70ce70ce50c0000000000000000000000000000000000000000000000000000000000000000ffff0000e50c06000d00440042004500420045004200450042004600440045004300460044004600440000000000000000000000000000000000000000000000000000000000000000003c01000000004ace0000c8c30600cb20000033013101000008000000003f32390200708203003e0000009ec3dc0064000000c981b00001010000000000000000000000000000ff0001000000b6031600000044303e4000000000a1140000000101010006010026b00900000000003c0136013701ba03f3b1950af50200008051010000000301000000000000000000feff7fdc2f0101b00f000000a7";
console.log(msg.length)

const pattern = new RegExp([
"55aaeb900200",
"(?<cell_0_voltage>....)",
"(?<cell_1_voltage>....)",
"(?<cell_2_voltage>....)",
"(?<cell_3_voltage>....)",
"(?<cell_4_voltage>....)",
"(?<cell_5_voltage>....)",
"(?<cell_6_voltage>....)",
"(?<cell_7_voltage>....)",
"(?<cell_8_voltage>....)",
"(?<cell_9_voltage>....)",
"(?<cell_10_voltage>....)",
"(?<cell_11_voltage>....)",
"(?<cell_12_voltage>....)",
"(?<cell_13_voltage>....)",
"(?<cell_14_voltage>....)",
"(?<cell_15_voltage>....)",
"....".repeat(16), // 16-31
"(?<cell_status>........)",
"(?<cell_voltage_average>....)",
"(?<cell_voltage_difference_max>....)",
"(?<cell_number_with_max_voltage>..)",
"(?<cell_number_with_min_voltage>..)",
"(?<cell_0_resistance>....)",
"(?<cell_1_resistance>....)",
"(?<cell_2_resistance>....)",
"(?<cell_3_resistance>....)",
"(?<cell_4_resistance>....)",
"(?<cell_5_resistance>....)",
"(?<cell_6_resistance>....)",
"(?<cell_7_resistance>....)",
"(?<cell_8_resistance>....)",
"(?<cell_9_resistance>....)",
"(?<cell_10_resistance>....)",
"(?<cell_11_resistance>....)",
"(?<cell_12_resistance>....)",
"(?<cell_13_resistance>....)",
"(?<cell_14_resistance>....)",
"(?<cell_15_resistance>....)",
"....".repeat(16), // 16-31
"(?<mos_temperature>....)",
"(?<cell_wire_resistance_status>........)",
"(?<battery_voltage>........)",
"(?<battery_watt>........)",
"(?<battery_current>........)",
"(?<battery_temperature_1>....)",
"(?<battery_temperature_2>....)",
"(?<alarms>........)",
"(?<balance_current>....)",
"(?<balance_state>..)",
"(?<state_of_charge>..)",
"(?<remaining_capacity>........)", // mAh
"(?<full_charge_capacity>........)", // mAh
"(?<cycle_count>........)",
"(?<total_cycle_capacity>........)", // mAh
"(?<state_of_health>..)",
"(?<precharge>..)",
"(?<user_alarm>....)",
"(?<runtime>........)",
"(?<charge>..)",
"(?<discharge>..)",
"(?<user_alarm_2>....)",
"(?<discharge_overcurrent_protection_release_time>....)",
"(?<discharge_short_circuit_protection_release_time>....)",
"(?<charge_overcurrent_protection_release_time>....)",
"(?<charge_short_circuit_protection_release_time>....)",
"(?<undervoltage_protection_release_time>....)",
"(?<overvoltage_protection_release_time>....)",
"(?<temperature_sensor_status>..)",
"(?<heating>..)",
"(?<reserved>....)",
"(?<emergency_switch_time>....)",
"(?<discharge_current_correction_factor>....)",
"(?<charging_current_sensor_voltage>....)",
"(?<discharging_current_sensor_voltage>....)",
"(?<battery_voltage_correction_factor>........)",
"........", // 2 bytes are skipped
"(?<battery_voltage_2>....)",
"(?<heating_current>....)",
"............",
"(?<reserved_2>..)",
"(?<charger_plugged>..)",
"(?<system_runtime_ticks>........)",
"........", // 2 bytes are skipped
"(?<battery_temperature_3>....)",
"(?<battery_temperature_4>....)",
"(?<battery_temperature_5>....)",
"....",
"(?<rtc_counter>........)",
"........",
"(?<time_enter_sleep>........)",
"(?<parallel_current_limiting_status>..)",
"(?<reserved_3>..)",
"(?<trailer>..............................................)",
"(?<maybecrc>..)"
].join(""));

const parseToBits = (hex) => {
	return [...hex.match(/(..)/g)].map((b) => {
		// reverse because little endian 
		return Number(parseInt(b, 16)).toString(2).padStart(8, "0").split("").toReversed();
	}).flat().map((a) => a === "1");
}
const parseUint8 = (hex) => {
	return new DataView(new Uint8Array(hex.match(/(..)/g)!.map((b) => parseInt(b, 16))).buffer).getUint8(0, true);
}
const parseUint16 = (hex) => {
	return new DataView(new Uint8Array(hex.match(/(..)/g)!.map((b) => parseInt(b, 16))).buffer).getUint16(0, true);
}
const parseInt16 = (hex) => {
	return new DataView(new Uint8Array(hex.match(/(..)/g)!.map((b) => parseInt(b, 16))).buffer).getInt16(0, true);
}
const parseInt32 = (hex) => {
	return new DataView(new Uint8Array(hex.match(/(..)/g)!.map((b) => parseInt(b, 16))).buffer).getInt32(0, true);
}
const parseFloat32 = (hex) => {
	return new DataView(new Uint8Array(hex.match(/(..)/g)!.map((b) => parseInt(b, 16))).buffer).getFloat32(0, true);
}
const parseUint32 = (hex) => {
	return new DataView(new Uint8Array(hex.match(/(..)/g)!.map((b) => parseInt(b, 16))).buffer).getUint32(0, true);
}

const parse = (matched) => ({
	cell_0_voltage: parseUint16(matched.cell_0_voltage)/1000,
	cell_1_voltage: parseUint16(matched.cell_1_voltage)/1000,
	cell_2_voltage: parseUint16(matched.cell_2_voltage)/1000,
	cell_3_voltage: parseUint16(matched.cell_3_voltage)/1000,
	cell_4_voltage: parseUint16(matched.cell_4_voltage)/1000,
	cell_5_voltage: parseUint16(matched.cell_5_voltage)/1000,
	cell_6_voltage: parseUint16(matched.cell_6_voltage)/1000,
	cell_7_voltage: parseUint16(matched.cell_7_voltage)/1000,
	cell_8_voltage: parseUint16(matched.cell_8_voltage)/1000,
	cell_9_voltage: parseUint16(matched.cell_9_voltage)/1000,
	cell_10_voltage: parseUint16(matched.cell_10_voltage)/1000,
	cell_11_voltage: parseUint16(matched.cell_11_voltage)/1000,
	cell_12_voltage: parseUint16(matched.cell_12_voltage)/1000,
	cell_13_voltage: parseUint16(matched.cell_13_voltage)/1000,
	cell_14_voltage: parseUint16(matched.cell_14_voltage)/1000,
	cell_15_voltage: parseUint16(matched.cell_15_voltage)/1000,
	cell_status: parseToBits(matched.cell_status),
	cell_voltage_average: parseUint16(matched.cell_voltage_average)/1000,
	cell_voltage_difference_max: parseUint16(matched.cell_voltage_difference_max)/1000,
	cell_number_with_max_voltage: parseUint8(matched.cell_number_with_max_voltage),
	cell_number_with_min_voltage: parseUint8(matched.cell_number_with_min_voltage),
	cell_0_resistance: parseUint16(matched.cell_0_resistance)/1000,
	cell_1_resistance: parseUint16(matched.cell_1_resistance)/1000,
	cell_2_resistance: parseUint16(matched.cell_2_resistance)/1000,
	cell_3_resistance: parseUint16(matched.cell_3_resistance)/1000,
	cell_4_resistance: parseUint16(matched.cell_4_resistance)/1000,
	cell_5_resistance: parseUint16(matched.cell_5_resistance)/1000,
	cell_6_resistance: parseUint16(matched.cell_6_resistance)/1000,
	cell_7_resistance: parseUint16(matched.cell_7_resistance)/1000,
	cell_8_resistance: parseUint16(matched.cell_8_resistance)/1000,
	cell_9_resistance: parseUint16(matched.cell_9_resistance)/1000,
	cell_10_resistance: parseUint16(matched.cell_10_resistance)/1000,
	cell_11_resistance: parseUint16(matched.cell_11_resistance)/1000,
	cell_12_resistance: parseUint16(matched.cell_12_resistance)/1000,
	cell_13_resistance: parseUint16(matched.cell_13_resistance)/1000,
	cell_14_resistance: parseUint16(matched.cell_14_resistance)/1000,
	cell_15_resistance: parseUint16(matched.cell_15_resistance)/1000,
	mos_temperature: parseInt16(matched.mos_temperature)/10,
	cell_wire_resistance_status: parseToBits(matched.cell_wire_resistance_status),
	battery_voltage: parseUint32(matched.battery_voltage)/1000,
	battery_watt: parseUint32(matched.battery_watt)/1000,
	battery_current: parseInt32(matched.battery_current)/1000,
	battery_temperature_1: parseInt16(matched.battery_temperature_1)/10,
	battery_temperature_2: parseInt16(matched.battery_temperature_2)/10,
	alarms: parseToBits(matched.alarms).map((v, i) => {
		return {[[
			"resistence_of_the_balancing_wire_too_large",
			"MOS_overtemperature_protection",
			"the_number_of_cells_does_not_match_the_set_value",
			"current_sensor_abnormality",
			"single_unit_overvoltage_protection",
			"battery_overvoltage_protection",
			"charging_overcurrent_protection",
			"charging_short_circuit_protection",
			"charging_over_temperature_protection",
			"charging_low_temperature_protection",
			"internal_communication_abnormality",
			"single_unit_undervoltage_protection",
			"battery_undervoltage_protection",
			"discharge_overcurrent_protection",
			"discharge_short_circuit_protection",
			"discharge_over_temperature_protection",
			"charging_anomality",
			"discharge_anomality",
			"GPS_disconnected",
			"please_change_the_authorization_password",
			"discharge_on_failure",
			"battery_over_temperature",
			"temperature_sensor_anomaly",
			"parallel_module_failure",
			"unknown_1",
			"unknown_2",
			"unknown_3",
			"unknown_4",
			"unknown_5",
			"unknown_6",
			"unknown_7",
			"unknown_8",
		][i]]: v}
	}),
	balance_current: parseInt16(matched.balance_current)/1000,
	balance_state: [parseUint8(matched.balance_state)].map((a) => a === 0 ? "off" : a === 1 ? "charge" : "discharge")[0],
	state_of_charge: parseUint8(matched.state_of_charge),
	remaining_capacity: parseInt32(matched.remaining_capacity)/1000,
	full_charge_capacity: parseUint32(matched.full_charge_capacity)/1000,
	cycle_count: parseUint32(matched.cycle_count),
	total_cycle_capacity: parseUint32(matched.total_cycle_capacity)/1000,
	state_of_health: parseUint8(matched.state_of_health),
	precharge: parseUint8(matched.precharge),
	user_alarm: parseUint16(matched.user_alarm),
	runtime: parseUint32(matched.runtime)/60/60/24,
	charge: parseUint8(matched.charge),
	discharge: parseUint8(matched.discharge),
	user_alarm_2: parseUint16(matched.user_alarm_2),
	discharge_overcurrent_protection_release_time: parseUint16(matched.discharge_overcurrent_protection_release_time),
	discharge_short_circuit_protection_release_time: parseUint16(matched.discharge_short_circuit_protection_release_time),
	charge_overcurrent_protection_release_time: parseUint16(matched.charge_overcurrent_protection_release_time),
	charge_short_circuit_protection_release_time: parseUint16(matched.charge_short_circuit_protection_release_time),
	undervoltage_protection_release_time: parseUint16(matched.undervoltage_protection_release_time),
	overvoltage_protection_release_time: parseUint16(matched.overvoltage_protection_release_time),
	temperature_sensor_status: parseToBits(matched.temperature_sensor_status).map((v, i) => {
		return {[[
			"MOS_temperature_sensor",
			"battery_temperature_sensor_1",
			"battery_temperature_sensor_2",
			"battery_temperature_sensor_3",
			"battery_temperature_sensor_4",
			"battery_temperature_sensor_5",
			"unknown1",
			"unknown2",
		][i]]: v}
	}),
	heating: parseUint8(matched.heating),
	reserved: parseUint16(matched.reserved),
	emergency_switch_time: parseUint16(matched.emergency_switch_time),
	discharge_current_correction_factor: parseUint16(matched.discharge_current_correction_factor),
	charging_current_sensor_voltage: parseUint16(matched.charging_current_sensor_voltage)/1000,
	discharging_current_sensor_voltage: parseUint16(matched.discharging_current_sensor_voltage)/1000,
	battery_voltage_correction_factor: parseFloat32(matched.battery_voltage_correction_factor),
	battery_voltage_2: parseUint16(matched.battery_voltage_2)/100,
	heating_current: parseInt16(matched.heating_current)/1000,
	reserved_2: parseUint8(matched.reserved_2),
	charger_plugged: parseUint8(matched.charger_plugged),
	system_runtime_ticks: parseUint32(matched.system_runtime_ticks)/10/60/60/24,
	battery_temperature_3: parseInt16(matched.battery_temperature_3)/10,
	battery_temperature_4: parseInt16(matched.battery_temperature_4)/10,
	battery_temperature_5: parseInt16(matched.battery_temperature_5)/10,
	rtc_counter: parseUint32(matched.rtc_counter),
	time_enter_sleep: parseUint32(matched.time_enter_sleep),
	parallel_current_limiting_status: parseUint8(matched.parallel_current_limiting_status),
	reserved_3: parseUint8(matched.reserved_3),
	trailer: matched.trailer,
});

//const matched = msgs.match(pattern)?.groups!;

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
