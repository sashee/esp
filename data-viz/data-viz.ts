import {DatabaseSync} from "node:sqlite";
import path from "node:path";
import express from "express";
import compression from "compression";
import {Piscina} from "piscina";
import {statements, database} from "./queries.ts";

const piscina = new Piscina({
  // The URL must be a file:// URL
  filename: new URL('./worker.ts', import.meta.url).href
});

const app = express();
app.use(compression());
const port = 8080;


console.log(statements.moving_sum.all({from: 1755445318570, to: 1756050118570, value: "$.inverter.qpigs.pv_charging_power1", over: 1000 * 60 * 60 * 24}));

const latestValue = JSON.parse(database.prepare("SELECT value FROM data ORDER BY timestamp desc LIMIT 1;").get().value);
//console.log(latestValue);
console.log("");

Object.entries(latestValue).flatMap(([tl, tv]) => {
	return Object.entries(tv).flatMap(([ll, lv]) => {
		return Object.entries(lv).flatMap(([kl, kv]) => {
		if (["number", "boolean"].includes(typeof kv)) {
			//console.log(`<div class="chart"\n\tdata-series-1='{"type": "simple", "title": "${tl}/${ll}/${kl}", "params": {"value": "$.${tl}.${ll}.${kl}"}}'\n\tdata-title="${tl}/${ll}/${kl}"\n></div>`)
		}else {
			if (typeof kv === "object" && Object.values(kv).every((v) => typeof v === "boolean")) {
					//console.log(`<div class="chart"\n\t${Object.keys(kv).map((v,i) => `data-series-${i+1}='{"type": "simple", "title": "${tl}/${ll}/${kl}/${v}", "params": {"value": "$.${tl}.${ll}.${kl}${v.match(/^\d/) ? `[${v}]` : `.${v}`}"}}'`).join("\n\t")}\n\tdata-title="${tl}/${ll}/${kl}"\n></div>`)
			}else {
				if (typeof kv === "string" && kl === "balance_state") {
					//console.log(`<div class="chart"\n\tdata-series-1='{"type": "balance_state", "title": "${tl}/${ll}/${kl}", "params": {"value": "$.${tl}.${ll}.${kl}"}}'\n\tdata-title="${tl}/${ll}/${kl}"\n></div>`)
				}else {
					//console.log(tl, ll, kl, typeof kv, kv, Array.isArray(kv))
				}
			}
		}
		})
	})
})

//console.log(database.prepare("SELECT json_extract(value, '$.battery.bms.trailer') as value FROM data ORDER BY timestamp desc;").all());

app.use(express.static('static'))

app.get('/sql', async (req, res) => {
	console.log(req.query.sql)
	console.log(req.query.parameters)
	res.send(await piscina.run(req.query));
  //res.send(statements[req.query.sql].all(JSON.parse(req.query.parameters)))
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


