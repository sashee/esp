import {DatabaseSync} from "node:sqlite";
import path from "node:path";
import express from "express";

const app = express();
const port = 8080;

const dbPath = "data.db";
//const dbPath = path.join(process.env.HOME, "monitoring-data", "data.db");

const database = new DatabaseSync(dbPath, {readOnly: true});

const statements = {
	simple: database.prepare("SELECT timestamp, json_extract(value, :value) as data FROM data WHERE data is not null AND timestamp >= :from AND timestamp <= :to;")
};

const latestValue = JSON.parse(database.prepare("SELECT value FROM data ORDER BY timestamp desc LIMIT 1;").get().value);
//console.log(latestValue);

Object.entries(latestValue).flatMap(([tl, tv]) => {
	return Object.entries(tv).flatMap(([ll, lv]) => {
		return Object.entries(lv).flatMap(([kl, kv]) => {
		if (["number", "boolean"].includes(typeof kv)) {
			//console.log(`<div class="chart"\n\tdata-series-1='{"type": "simple", "title": "${tl}/${ll}/${kl}", "params": {"value": "$.${tl}.${ll}.${kl}"}}'\n\tdata-title="${tl}/${ll}/${kl}"\n></div>`)
		}else {
			if (typeof kv === "object" && Object.values(kv).every((v) => typeof v === "boolean")) {
				Object.keys(kv).map((v) => {
					//console.log(`<div class="chart"\n\tdata-series-1='{"type": "simple", "title": "${tl}/${ll}/${kl}/${v}", "params": {"value": "$.${tl}.${ll}.${kl}[${v}]"}}'\n\tdata-title="${tl}/${ll}/${kl}/${v}"\n></div>`)
				})
			}else {
			console.log(tl, ll, kl, typeof kv, kv, Array.isArray(kv))

			}
		}
		})
	})
})

app.use(express.static('static'))

app.get('/sql', (req, res) => {
	console.log(req.query.sql)
	console.log(req.query.parameters)
  res.send(statements[req.query.sql].all(JSON.parse(req.query.parameters)))
});

await new Promise((resolve, reject) => {
	app.listen(port, (e) => {
		if (e) {
			reject(e);
		}
		console.log(`Example app listening on port ${port}`)
	});
	app.on('close', () => resolve(undefined));
});


