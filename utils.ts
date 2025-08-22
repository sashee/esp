// https://stackoverflow.com/a/75806068
const crc_xmodem = (str: Uint8Array) => {
	let crc = 0;
	let xorout = 0;
    for(let i = 0, t: number; i < str.length; i++, crc &= 0xFFFF) {
        t = (crc >> 8) ^ str[i];
        t ^= t >> 4;
        crc = (crc << 8) ^ (t << 12) ^ (t << 5) ^ t;
    }
    const res = crc ^ xorout;
		return new Uint8Array([res >> 8, res & 0xFF]);
};

// uses a slightly different CRC where some bytes are increased
// https://github.com/ned-kelly/docker-voltronic-homeassistant/blob/master/sources/inverter-cli/inverter.cpp
export const modifiedCrc = (str: Uint8Array) => {
	const correctCrc = (crc: Uint8Array) => crc.map((v) => [0x28, 0x0D, 0x0A].includes(v) ? v + 1 : v);
	return correctCrc(crc_xmodem(str));
}

export const commands = [
	{
		command: "QPIGS",
		length: 106,
		parse: (message: string) => {
			const regex = /^(?<grid_voltage>\d\d\d\.\d) (?<grid_frequency>\d\d\.\d) (?<ac_output_voltage>\d\d\d\.\d) (?<ac_output_frequency>\d\d\.\d) (?<ac_output_apparent_power>\d\d\d\d) (?<ac_output_active_power>\d\d\d\d) (?<output_load_percent>\d\d\d) (?<bus_voltage>\d\d\d) (?<battery_voltage>\d\d\.\d\d) (?<battery_charging_current>\d\d\d) (?<battery_capacity>\d\d\d) (?<inverter_heat_sink_temperature>\d\d\d\d) (?<pv_input_current1>\d\d\.\d) (?<pv_input_voltage1>\d\d\d\.\d) (?<battery_voltage_from_scc1>\d\d\.\d\d) (?<battery_discharge_current>\d\d\d\d\d) (?<add_sbu_priority_version>[01])(?<configuration_status>[01])(?<scc_firmware_version>[01])(?<load_status>[01])(?<battery_voltage_to_steady_while_charging>[01])(?<charging_status>[01])(?<charging_status_scc_1>[01])(?<charging_status_ac>[01]) (?<battery_voltage_from_fans_on>\d\d) (?<eeprom_version>\d\d) (?<pv_charging_power1>\d\d\d\d\d) (?<flag_for_charging_to_flating_mode>[01])(?<switch_on>[01])(?<device_status_2_reserved>[01])$/;
			const match = message.match(regex);
			if (match) {
				const x = match.groups!;
				return {
					grid_voltage: Number(x["grid_voltage"]),
					grid_frequency: Number(x["grid_frequency"]),
					ac_output_voltage: Number(x["ac_output_voltage"]),
					ac_output_frequency: Number(x["ac_output_frequency"]),
					ac_output_apparent_power: Number(x["ac_output_apparent_power"]),
					ac_output_active_power: Number(x["ac_output_active_power"]),
					output_load_percent: Number(x["output_load_percent"]),
					bus_voltage: Number(x["bus_voltage"]),
					battery_voltage: Number(x["battery_voltage"]),
					battery_charging_current: Number(x["battery_charging_current"]),
					battery_capacity: Number(x["battery_capacity"]),
					inverter_heat_sink_temperature: Number(x["inverter_heat_sink_temperature"]),
					pv_input_current1: Number(x["pv_input_current1"]),
					pv_input_voltage1: Number(x["pv_input_voltage1"]),
					battery_voltage_from_scc1: Number(x["battery_voltage_from_scc1"]),
					battery_discharge_current: Number(x["battery_discharge_current"]),
					add_sbu_priority_version: Number(x["add_sbu_priority_version"]) == 1,
					configuration_status: Number(x["configuration_status"]) == 1,
					scc_firmware_version: Number(x["scc_firmware_version"]) == 1,
					load_status: Number(x["load_status"]) == 1,
					battery_voltage_to_steady_while_charging: Number(x["battery_voltage_to_steady_while_charging"]) == 1,
					charging_status: Number(x["charging_status"]) == 1,
					charging_status_scc_1: Number(x["charging_status_scc_1"]) == 1,
					charging_status_ac: Number(x["charging_status_ac"]) == 1,
					battery_voltage_from_fans_on: Number(x["battery_voltage_from_fans_on"]),
					eeprom_version: Number(x["eeprom_version"]),
					pv_charging_power1: Number(x["pv_charging_power1"]),
					flag_for_charging_to_flating_mode: Number(x["flag_for_charging_to_flating_mode"]) == 1,
					switch_on: Number(x["switch_on"]) == 1,
					device_status_2_reserved: Number(x["device_status_2_reserved"]) == 1,
				}
			}
		},
	},
	{
		command: "QPIGS2",
		length: 17,
		parse: (message: string) => {
			const regex = /^(?<pv_input_current2>\d\d\.\d) (?<pv_input_voltage2>\d\d\d\.\d) (?<pv_charging_power2>\d\d\d\d\d) $/;
			const match = message.match(regex);
			if (match) {
				const x = match.groups!;
				return {
					pv_input_current2: Number(x["pv_input_current2"]),
					pv_input_voltage2: Number(x["pv_input_voltage2"]),
					pv_charging_power2: Number(x["pv_charging_power2"]),
				};
			}
		},
	},
	{
		command: "QPIWS",
		length: 36,
		parse: (message: string) => {
			const regex = /^(?<reserved1>[01])(?<inverter_fault>[01])(?<bus_over>[01])(?<bus_under>[01])(?<bus_soft_fail>[01])(?<line_fail>[01])(?<opvshort>[01])(?<inverter_voltage_too_low>[01])(?<inverter_voltage_too_high>[01])(?<over_temperature>[01])(?<fan_locked>[01])(?<battery_voltage_high>[01])(?<battery_low_alarm>[01])(?<reserved_overcharge>[01])(?<battery_under_shutdown>[01])(?<reserved_battery_derating>[01])(?<over_load>[01])(?<eeprom_fault>[01])(?<inverter_over_current>[01])(?<inverter_soft_fail>[01])(?<self_test_fail>[01])(?<op_dv_voltage_over>[01])(?<bat_open>[01])(?<current_sensor_fail>[01])(?<battery_short>[01])(?<power_limit>[01])(?<pv_voltage_high_1>[01])(?<mppt_overload_fault_1>[01])(?<mppt_overload_warning_1>[01])(?<battery_too_low_to_charge_1>[01])(?<pv_voltage_high_2>[01])(?<mppt_overload_fault_2>[01])(?<mppt_overload_warning_2>[01])(?<battery_too_low_to_charge_2>[01])(?<unknown1>[01])(?<unknown2>[01])$/;
			const match = message.match(regex);
			if (match) {
				const x = match.groups!;
				return {
					reserved1: Number(x["reserved1"]) == 1,
					inverter_fault: Number(x["inverter_fault"]) == 1,
					bus_over: Number(x["bus_over"]) == 1,
					bus_under: Number(x["bus_under"]) == 1,
					bus_soft_fail: Number(x["bus_soft_fail"]) == 1,
					line_fail: Number(x["line_fail"]) == 1,
					opvshort: Number(x["opvshort"]) == 1,
					inverter_voltage_too_low: Number(x["inverter_voltage_too_low"]) == 1,
					inverter_voltage_too_high: Number(x["inverter_voltage_too_high"]) == 1,
					over_temperature: Number(x["over_temperature"]) == 1,
					fan_locked: Number(x["fan_locked"]) == 1,
					battery_voltage_high: Number(x["battery_voltage_high"]) == 1,
					battery_low_alarm: Number(x["battery_low_alarm"]) == 1,
					reserved_overcharge: Number(x["reserved_overcharge"]) == 1,
					battery_under_shutdown: Number(x["battery_under_shutdown"]) == 1,
					reserved_battery_derating: Number(x["reserved_battery_derating"]) == 1,
					over_load: Number(x["over_load"]) == 1,
					eeprom_fault: Number(x["eeprom_fault"]) == 1,
					inverter_over_current: Number(x["inverter_over_current"]) == 1,
					inverter_soft_fail: Number(x["inverter_soft_fail"]) == 1,
					self_test_fail: Number(x["self_test_fail"]) == 1,
					op_dv_voltage_over: Number(x["op_dv_voltage_over"]) == 1,
					bat_open: Number(x["bat_open"]) == 1,
					current_sensor_fail: Number(x["current_sensor_fail"]) == 1,
					battery_short: Number(x["battery_short"]) == 1,
					power_limit: Number(x["power_limit"]) == 1,
					pv_voltage_high_1: Number(x["pv_voltage_high_1"]) == 1,
					mppt_overload_fault_1: Number(x["mppt_overload_fault_1"]) == 1,
					mppt_overload_warning_1: Number(x["mppt_overload_warning_1"]) == 1,
					battery_too_low_to_charge_1: Number(x["battery_too_low_to_charge_1"]) == 1,
					pv_voltage_high_2: Number(x["pv_voltage_high_2"]) == 1,
					mppt_overload_fault_2: Number(x["mppt_overload_fault_2"]) == 1,
					mppt_overload_warning_2: Number(x["mppt_overload_warning_2"]) == 1,
					battery_too_low_to_charge_2: Number(x["battery_too_low_to_charge_2"]) == 1,
					unknown1: Number(x["unknown1"]) == 1,
					unknown2: Number(x["unknown2"]) == 1,
				};
			}
		},
	}
];

export const arrToHex = (arr: Uint8Array) => {
	return [...arr].map((x) => x.toString(16).padStart(2, '0')).join('')
};

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

const bmsMatcher = new RegExp([
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

export const bms = {
	length: 300,
	parse: (values: Uint8Array) => {
		const hex = [...values].map((a) => a.toString(16).padStart(2, "0")).join("");

		const matched = hex.match(bmsMatcher)?.groups;
		if (matched) {
			return {
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
			};
		}
	}
};

export const processLogs = (contents: string) => {
	const numbers = contents.split("\n").join("").split(/(..)/).filter((s: string) => s !== "").map((h: string) => parseInt(h, 16));

	const checkCrc = (numbers: Uint8Array) => {
		const calculatedCrc = crc_xmodem(new Uint8Array(numbers.slice(0, -2)));
		const actualCrc = numbers.slice(-2);
		return calculatedCrc.length === actualCrc.length && calculatedCrc.every((v, i) => v === actualCrc[i]);
	};

	const messages = numbers.reduce(({skipTo, results}, e, i) => {
		if (i < skipTo) {
			return {skipTo, results};
		}else {
			if (e === 40) {
				const matchedCommands = commands.flatMap((command) => {
					if (numbers[i + command.length + 3] !== 13) {
						return [];
					}
					const bytes = new Uint8Array(numbers.slice(i, i + command.length + 4));
					const crcOk = checkCrc(bytes.subarray(0, -1));
					if (!crcOk) {
						return [];
					}
					const parsed = command.parse(new TextDecoder().decode(bytes.subarray(1, -3)));
					if (!parsed) {
						return [];
					}else {
						return [{
							from: i,
							to: i + command.length + 3,
							command: command.command,
							values: parsed,
							bytes,
						}];
					}
				});
				if (matchedCommands.length > 1) {
					console.warn("multiple commands match start index");
				}
				if (matchedCommands.length > 0) {
					return {
						skipTo: matchedCommands[0].to + 1,
						results: [...results, matchedCommands[0]],
					};
				}else {
					return {skipTo, results};
				}
			}else {
				return {skipTo, results};
			}
		}
	}, {skipTo: 0, results: []}).results;

	return {
		messages,
		length: numbers.length,
	};
}

