import {DatabaseSync} from "node:sqlite";
import path from "node:path";

export const database = (() => {
	try {
		return new DatabaseSync(path.join(process.env.HOME, "monitoring-data", "data.db"), {readOnly: true});
	}catch(e) {
		return new DatabaseSync("data.db", {readOnly: true});
	}
})();

const makeMovingSum = (prepareDataQuery: string) => `
SELECT
	timestamp, data
FROM (
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
				${prepareDataQuery}
			)
		)
		WHERE
			prev is not null AND
			next is not null AND
			abs(next_timestamp-timestamp) < 15*60*1000 AND
			abs(prev_timestamp-timestamp) < 15*60*1000
	)
)
WHERE
	timestamp >= :from AND timestamp <= :to
;`;

export const statements = {
	simple: database.prepare("SELECT timestamp, json_extract(value, :value) as data FROM data WHERE data is not null AND timestamp >= :from AND timestamp <= :to;"),
	three_state_value: database.prepare("SELECT timestamp, CASE json_extract(value, :value) WHEN :zero then 0 WHEN :one THEN 1 ELSE 2 END as data FROM data WHERE data is not null AND timestamp >= :from AND timestamp <= :to;"),
	sum_two: database.prepare("SELECT timestamp, (ifnull(json_extract(value, :value1), 0) + ifnull(json_extract(value, :value2), 0)) as data FROM data WHERE data is not null AND timestamp >= :from AND timestamp <= :to;"),
	moving_sum: database.prepare(makeMovingSum("SELECT timestamp, json_extract(value, :value) as data FROM data WHERE data is not null")),
	moving_sum_sum_two: database.prepare(makeMovingSum("SELECT timestamp, (ifnull(json_extract(value, :value1), 0) + ifnull(json_extract(value, :value2), 0)) as data FROM data WHERE data is not null")),
};
