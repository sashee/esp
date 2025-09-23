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

const sumTwoData = (query: string) => `
SELECT timestamp, ifnull(data1, 0) + ifnull(data2, 0) as data FROM (${increaseIndent(query)})
`;

const threestate = (zeroNamedParameter: string, oneNamedParameter: string) => (query: string) => `
SELECT timestamp, CASE data WHEN :${zeroNamedParameter} then 0 WHEN :${oneNamedParameter} THEN 1 ELSE 2 END as data FROM (${increaseIndent(query)})
`;

const multipleWithSign = (dataName: string, signName: string) => (query: string) => `
SELECT timestamp, (${dataName} * ifnull(sign(${signName}), 0)) as data FROM (${increaseIndent(query)})
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

const queries = {
	simple: flow(extractFromData("value"), dropNullData(), timerange),
	three_state_value: flow(extractFromData("value"), dropNullData(), threestate("zero", "one"), timerange),
	sum_two: flow(extractTwoFromData("value1", "value2"), sumTwoData, timerange),
	moving_sum: flow(extractFromData("value"), dropNullData(), makeMovingSum, timerange),
	moving_sum_sum_two: flow(extractTwoFromData("value1", "value2"), sumTwoData, makeMovingSum, timerange),
	simple_with_sign: flow(extractTwoFromData("value", "sign"), dropNullData("data1"), multipleWithSign("data1", "data2"), timerange),
	moving_sum_with_sign: flow(extractTwoFromData("value", "sign"), dropNullData("data1"), multipleWithSign("data1", "data2"), makeMovingSum, timerange),
	derivation: flow(extractFromData("value"), dropNullData(), derivation, timerange),
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

