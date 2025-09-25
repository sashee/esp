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

const extractFromData = (namedParameter: string) => () => `
SELECT timestamp, json_extract(value, :${namedParameter}) as data FROM data
`;

const dropNullData = (dataName: string = "data") => (query: string) => `
SELECT * FROM (${increaseIndent(query)}) where ${dataName} is not null
`;

const extractTwoFromData = (namedParameter1: string, namedParameter2: string) => () => `
SELECT timestamp, json_extract(value, :${namedParameter1}) as data1, json_extract(value, :${namedParameter2}) as data2 FROM data
`;

const extractFiveFromData = (resultColum1: string = "data1", resultColum2: string = "data2", resultColum3: string = "data3", resultColum4: string = "data4", resultColum5: string = "data5") => (namedParameter1: string, namedParameter2: string, namedParameter3: string, namedParameter4: string, namedParameter5: string) => () => `
SELECT timestamp, json_extract(value, :${namedParameter1}) as ${resultColum1}, json_extract(value, :${namedParameter2}) as ${resultColum2}, json_extract(value, :${namedParameter3}) as ${resultColum3}, json_extract(value, :${namedParameter4}) as ${resultColum4}, json_extract(value, :${namedParameter5}) as ${resultColum5} FROM data
`;

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
	(timestamp - prev_timestamp)*1.0 * (data - prev) /1000/60/60/24 as data
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
	battery_watt: [extractTwoFromData("value", "sign"), dropNullData("data1"), multipleWithSign()("data1", "data2"), multiply()(-1)],
	pv: [extractFromData("value"), dropNullData()],
	pv_sum: [extractTwoFromData("value1", "value2"), sumTwoData()("data1", "data2")],
	active_power: [extractFromData("value"), dropNullData(), multiply()(-1)],
	sum: [extractFiveFromData("pv1", "pv2", "battery_watt", "battery_sign", "ac_output")("pv1", "pv2", "battery_watt", "battery_sign", "ac_output"), sumTwoData(["battery_watt", "battery_sign", "ac_output"], "pv_sum")("pv1", "pv2"), multipleWithSign(["pv_sum", "ac_output"], "battery_watt_signed")("battery_watt", "battery_sign"), multiply(["pv_sum", "ac_output"], "battery_watt_signed")(-1), multiply(["pv_sum", "battery_watt_signed"], "ac_output")(-1), sumThreeData([], "data")("pv_sum", "battery_watt_signed", "ac_output")],

};

const queries = {
	simple: flow(extractFromData("value"), dropNullData(), timerange),
	three_state_value: flow(extractFromData("value"), dropNullData(), threestate("zero", "one"), timerange),
	sum_two: flow(extractTwoFromData("value1", "value2"), sumTwoData()("data1", "data2"), timerange),
	moving_sum: flow(extractFromData("value"), dropNullData(), makeMovingSum, timerange),
	moving_sum_sum_two: flow(extractTwoFromData("value1", "value2"), sumTwoData()("data1", "data2"), makeMovingSum, timerange),
	simple_with_sign: flow(extractTwoFromData("value", "sign"), dropNullData("data1"), multipleWithSign()("data1", "data2"), timerange),
	moving_sum_with_sign: flow(extractTwoFromData("value", "sign"), dropNullData("data1"), multipleWithSign()("data1", "data2"), makeMovingSum, timerange),
	derivation: flow(extractFromData("value"), dropNullData(), derivation, timerange),
	telj_battery_watt: flow(...teljesitmenyek.battery_watt, round()(), timerange),
	telj_pv: flow(...teljesitmenyek.pv, round()(), timerange),
	telj_pv_sum: flow(...teljesitmenyek.pv_sum, round()(), timerange),
	telj_active_power: flow(...teljesitmenyek.active_power, round()(), timerange),
	telj_sum: flow(...teljesitmenyek.sum, round()(), timerange),
	telj_moving_sum_battery_watt: flow(...teljesitmenyek.battery_watt, makeMovingSum, round()(), timerange),
	telj_moving_sum_pv: flow(...teljesitmenyek.pv, makeMovingSum, round()(), timerange),
	telj_moving_sum_pv_sum: flow(...teljesitmenyek.pv_sum, makeMovingSum, round()(), timerange),
	telj_moving_sum_active_power: flow(...teljesitmenyek.active_power, makeMovingSum, round()(), timerange),
	telj_moving_sum_sum: flow(...teljesitmenyek.sum, makeMovingSum, round()(), timerange),
};

export const statements = Object.fromEntries(Object.entries(queries).map(([k, v]) => {
	try {
		const prepared = database.prepare(v + ";");
		return [k, prepared];
	}catch(e) {
		console.error(v);
		throw e;
	}
}));

