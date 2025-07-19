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

