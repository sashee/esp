// https://stackoverflow.com/a/75806068
export const crc_xmodem = (str: Uint8Array) => {
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

export const commands = [
	{
		command: "QPIGS",
		length: 106,
		parse: (message: string) => {
			const regex = /^(?<grid_voltage>\d\d\d\.\d) (?<grid_frequency>\d\d\.\d) (?<ac_output_voltage>\d\d\d\.\d) (?<ac_output_frequency>\d\d\.\d) (?<ac_output_apparent_power>\d\d\d\d) (?<ac_output_active_power>\d\d\d\d) (?<output_load_percent>\d\d\d) (?<bus_voltage>\d\d\d) (?<battery_voltage>\d\d\.\d\d) (?<battery_charging_current>\d\d\d) (?<battery_capacity>\d\d\d) (?<inverter_heat_sink_temperature>\d\d\d\d) (?<pv_input_current1>\d\d\.\d) (?<pv_input_voltage1>\d\d\d\.\d) (?<battery_voltage_from_scc1>\d\d\.\d\d) (?<battery_discharge_current>\d\d\d\d\d) (?<add_sbu_priority_version>[01])(?<configuration_status>[01])(?<scc_firmware_version>[01])(?<load_status>[01])(?<battery_voltage_to_steady_while_charging>[01])(?<charging_status>[01])(?<charging_status_scc_1>[01])(?<charging_status_ac>[01]) (?<battery_voltage_from_fans_on>\d\d) (?<eeprom_version>\d\d) (?<pv_charging_power1>\d\d\d\d\d) (?<flag_for_charging_to_flating_mode>[01])(?<switch_on>[01])(?<device_status_2_reserved>[01])$/;
			const match = message.match(regex);
			if (match) {
				return match.groups;
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
				return match.groups;
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
				return match.groups;
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

