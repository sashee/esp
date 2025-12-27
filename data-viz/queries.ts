import {DatabaseSync} from "node:sqlite";
import path from "node:path";

export const database = (() => {
	try {
		return new DatabaseSync(path.join(process.env.HOME, "monitoring-data", "data.db"), {readOnly: true});
	}catch(e) {
		return new DatabaseSync("data.db", {readOnly: true});
	}
})();

const increaseIndent = (str: string) => str.split("\n").map((l) => `\t${l}`).join("\n");

const makeMovingSum = (query: string) => `
SELECT
	timestamp,
	sum(rectangle_area) OVER (
		ORDER BY timestamp
		RANGE BETWEEN :over PRECEDING
		AND CURRENT ROW
	) as data
FROM (
	SELECT
		timestamp,
		(abs(timestamp - prev_timestamp) / 2 + abs(timestamp - next_timestamp) / 2) * data / 60 / 60 / 1000 as rectangle_area
	FROM (
		SELECT
			timestamp,
			data,
			LAG(data) OVER (ORDER BY timestamp asc) prev,
			LAG(timestamp) OVER (ORDER BY timestamp asc) prev_timestamp,
			LEAD(data) OVER (ORDER BY timestamp asc) next,
			LEAD(timestamp) OVER (ORDER BY timestamp asc) next_timestamp
		FROM (
			${increaseIndent(query)}
		)
	)
	WHERE
		prev is not null AND
		next is not null AND
		abs(next_timestamp-timestamp) < 15*60*1000 AND
		abs(prev_timestamp-timestamp) < 15*60*1000
)
`;

const flow = (...arr) => {
	return arr.reduce((memo, fn) => {
		return fn(memo);
	}, undefined);
};

const timerange = (query: string) => `
SELECT * FROM (${increaseIndent(query)}) WHERE timestamp >= :from AND timestamp <= :to
`;

const selectColumn = (paramName: string = "value") => () => {
	return `SELECT timestamp, ${paramName} as data FROM data`;
};

const dropNullData = (dataName: string = "data") => (query: string) => `
SELECT * FROM (${increaseIndent(query)}) where ${dataName} is not null
`;

const selectTwoColumns = (paramName1: string = "value1", paramName2: string = "value2") => () => {
	return `SELECT timestamp, ${paramName1} as data1, ${paramName2} as data2 FROM data`;
};

const selectFiveColumns = (paramName1: string = "pv1", paramName2: string = "pv2", paramName3: string = "battery_watt", paramName4: string = "battery_sign", paramName5: string = "ac_output") => (resultColum1: string = "data1", resultColum2: string = "data2", resultColum3: string = "data3", resultColum4: string = "data4", resultColum5: string = "data5") => () => {
	return `SELECT timestamp, ${paramName1} as ${resultColum1}, ${paramName2} as ${resultColum2}, ${paramName3} as ${resultColum3}, ${paramName4} as ${resultColum4}, ${paramName5} as ${resultColum5} FROM data`;
};

const multiply = (extraColumns: string[] = [], columnName: string = "data") => (amount: number) => (query: string) => `
SELECT ${["timestamp", `${columnName} * ${amount} as ${columnName}`, ...extraColumns].join(", ")} fROM (${increaseIndent(query)})
`;

const sumTwoData = (extraColumns: string[] = [], resultColumnName: string = "data") => (column1Name: string, column2Name: string) => (query: string) => `
SELECT ${["timestamp", `ifnull(${column1Name}, 0) + ifnull(${column2Name}, 0) as ${resultColumnName}`, ...extraColumns].join(", ")} FROM (${increaseIndent(query)})
`;

const sumThreeData = (extraColumns: string[] = [], resultColumnName: string = "data") => (column1Name: string, column2Name: string, column3Name: string) => (query: string) => `
SELECT ${["timestamp", `ifnull(${column1Name}, 0) + ifnull(${column2Name}, 0) + ifnull(${column3Name}, 0) as ${resultColumnName}`, ...extraColumns].join(", ")} FROM (${increaseIndent(query)})
`;

const threestate = (zeroNamedParameter: string, oneNamedParameter: string) => (query: string) => `
SELECT timestamp, CASE data WHEN :${zeroNamedParameter} then 0 WHEN :${oneNamedParameter} THEN 1 ELSE 2 END as data FROM (${increaseIndent(query)})
`;

const multipleWithSign = (extraColumns: string[] = [], resultColumnName: string = "data") => (dataName: string, signName: string) => (query: string) => `
SELECT ${["timestamp", `(${dataName} * ifnull(sign(${signName}), 0)) as ${resultColumnName}`, ...extraColumns].join(", ")} FROM (${increaseIndent(query)})
`;

const round = (extraColumns: string[] = [], resultColumnName: string = "data") => (columnName: string = "data", decimals: number = 0) => (query: string) => `
SELECT ${["timestamp", `round(${columnName}, ${decimals}) as ${resultColumnName}`, ...extraColumns].join(", ")} FROM (${increaseIndent(query)})
`;

const derivation = (query: string) => `
SELECT
	timestamp,
	(timestamp - prev_timestamp)*1.0 / (data - prev) /1000/60/60/24 as data
FROM (
	SELECT
		timestamp,
		data,
		LAG(data) OVER (ORDER BY timestamp asc) prev,
		LAG(timestamp) OVER (ORDER BY timestamp asc) prev_timestamp
	FROM (
		SELECT
			timestamp,
			data,
			LAG(data) OVER (ORDER BY timestamp asc) prev,
			LAG(timestamp) OVER (ORDER BY timestamp asc) prev_timestamp
		FROM (
			${increaseIndent(query)}
		)
	)
	WHERE
		data is not null AND
		data != prev
)
`;

const teljesitmenyek = {
	battery_watt: [selectTwoColumns("battery_bms_battery_watt", "battery_bms_battery_current"), dropNullData("data1"), multipleWithSign()("data1", "data2"), multiply()(-1)],
	pv: [selectColumn("inverter_qpigs_pv_charging_power1"), dropNullData()],
	pv_sum: [selectTwoColumns("inverter_qpigs_pv_charging_power1", "inverter_qpigs2_pv_charging_power2"), sumTwoData()("data1", "data2")],
	active_power: [selectColumn("inverter_qpigs_ac_output_active_power"), dropNullData(), multiply()(-1)],
	sum: [selectFiveColumns("inverter_qpigs_pv_charging_power1", "inverter_qpigs2_pv_charging_power2", "battery_bms_battery_watt", "battery_bms_battery_current", "inverter_qpigs_ac_output_active_power")("pv1", "pv2", "battery_watt", "battery_sign", "ac_output"), sumTwoData(["battery_watt", "battery_sign", "ac_output"], "pv_sum")("pv1", "pv2"), multipleWithSign(["pv_sum", "ac_output"], "battery_watt_signed")("battery_watt", "battery_sign"), multiply(["pv_sum", "ac_output"], "battery_watt_signed")(-1), multiply(["pv_sum", "battery_watt_signed"], "ac_output")(-1), sumThreeData([], "data")("pv_sum", "battery_watt_signed", "ac_output")],

};

const queries = {
	simple: (params: {_value: string}) => flow(selectColumn(params._value), dropNullData(), timerange),
	three_state_value: (params: {_value: string}) => flow(selectColumn(params._value), dropNullData(), threestate("zero", "one"), timerange),
	sum_two: (params: {_value1: string, _value2: string}) => flow(selectTwoColumns(params._value1, params._value2), dropNullData("data1"), sumTwoData()("data1", "data2"), timerange),
	moving_sum: (params: {_value: string}) => flow(selectColumn(params._value), dropNullData(), makeMovingSum, timerange),
	moving_sum_sum_two: (params: {_value1: string, _value2: string}) => flow(selectTwoColumns(params._value1, params._value2), dropNullData("data1"), sumTwoData()("data1", "data2"), makeMovingSum, timerange),
	simple_with_sign: (params: {_value: string, _sign: string}) => flow(selectTwoColumns(params._value, params._sign), dropNullData("data1"), multipleWithSign()("data1", "data2"), timerange),
	moving_sum_with_sign: (params: {_value: string, _sign: string}) => flow(selectTwoColumns(params._value, params._sign), dropNullData("data1"), multipleWithSign()("data1", "data2"), makeMovingSum, timerange),
	derivation: (params: {_value: string}) => flow(selectColumn(params._value), dropNullData(), derivation, timerange),
	telj_battery_watt: () => flow(...teljesitmenyek.battery_watt, round()(), timerange),
	telj_pv: () => flow(...teljesitmenyek.pv, round()(), timerange),
	telj_pv_sum: () => flow(...teljesitmenyek.pv_sum, round()(), timerange),
	telj_active_power: () => flow(...teljesitmenyek.active_power, round()(), timerange),
	telj_sum: () => flow(...teljesitmenyek.sum, round()(), timerange),
	telj_moving_sum_battery_watt: () => flow(...teljesitmenyek.battery_watt, makeMovingSum, round()(), timerange),
	telj_moving_sum_pv: () => flow(...teljesitmenyek.pv, makeMovingSum, round()(), timerange),
	telj_moving_sum_pv_sum: () => flow(...teljesitmenyek.pv_sum, makeMovingSum, round()(), timerange),
	telj_moving_sum_active_power: () => flow(...teljesitmenyek.active_power, makeMovingSum, round()(), timerange),
	telj_moving_sum_sum: () => flow(...teljesitmenyek.sum, makeMovingSum, round()(), timerange),
};

export const statements = queries;

