import path from "node:path";
import express from "express";
import type {Response} from "express";
import compression from "compression";
import {Piscina} from "piscina";
import {statements, database} from "./queries.ts";
import {buildInfoPanelRgb565, rgb565ToPng, runInfoPanelQuery} from "./info-panel.ts";

const setNoCacheHeaders = (res: Response) => {
  res.setHeader('Cache-Control', 'no-store, no-cache, must-revalidate, max-age=0');
  res.setHeader('Pragma', 'no-cache');
  res.setHeader('Expires', '0');
};

const piscina = new Piscina({
  // The URL must be a file:// URL
  filename: new URL('./worker.ts', import.meta.url).href,
	idleTimeout: 10000,
});

const app = express();
app.use(compression());
const port = 8080;

{
	/*
	const sql = statements.moving_sum;
  const params = {from: 1755445318570, to: 1756050118570, value: "inverter_qpigs_pv_charging_power1", over: 1000 * 60 * 60 * 24};
	*/
  const params = {from: 1755445318570, to: 1756050118570, _value: "inverter_qpigs_pv_charging_power1"};
	const sql = statements.simple(params);
	//console.log(sql(params));
  
	console.log(sql, params);
	const res = database.prepare(sql + ";").all(Object.fromEntries(Object.entries(params).filter(([k]) => !k.startsWith("_"))));
	console.log(res);

  const infoPanelTo = new Date(2026, 1, 11, 12, 0, 0, 0).getTime();
  const infoPanelParams = {
    from: infoPanelTo - 15 * 60 * 1000,
    to: infoPanelTo,
  };
  const infoPanelSql = statements.info_panel();
  console.log(infoPanelSql, infoPanelParams);
  const infoPanelRes = database.prepare(infoPanelSql + ";").all(infoPanelParams);
  console.log(infoPanelRes);
}

//const latestValue = JSON.parse(database.prepare("SELECT value FROM data ORDER BY timestamp desc LIMIT 1;").get().value);
//console.log(latestValue);
//console.log("");

/*
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
*/

//console.log(database.prepare("SELECT json_extract(value, '$.battery.bms.trailer') as value FROM data ORDER BY timestamp desc;").all());

app.use(express.static(path.join(import.meta.dirname, 'static')))

app.get('/info-panel.rgb565', (_req, res) => {
  setNoCacheHeaders(res);
	const panelWidth = 128;
	const panelHeight = 160;
  const now = new Date();
  const panelTime = true ? new Date(
    2026,
    1,
    11,
    now.getHours(),
    now.getMinutes(),
    now.getSeconds(),
    now.getMilliseconds(),
  ) : now;
  const infoPanelRow = runInfoPanelQuery(panelTime);
  const rgb565 = buildInfoPanelRgb565(panelWidth, panelHeight)(panelTime, infoPanelRow);
  res.status(200);
  res.setHeader('Content-Type', 'application/octet-stream');
  res.setHeader('Content-Length', String(rgb565.length));
  res.setHeader('X-Image-Format', 'rgb565be');
  res.setHeader('X-Image-Width', String(panelWidth));
  res.setHeader('X-Image-Height', String(panelHeight));
  res.send(rgb565);
});

app.get('/info-panel.png', async (req, res) => {
  setNoCacheHeaders(res);
  try {
    const host = req.get('host');
    if (!host) {
      res.status(400).send('missing Host header');
      return;
    }

    const rgb565Url = `${req.protocol}://${host}/info-panel.rgb565`;
    const upstream = await fetch(rgb565Url);
    if (!upstream.ok) {
      res.status(502).send(`upstream rgb565 failed with status ${upstream.status}`);
      return;
    }

    const format = upstream.headers.get('x-image-format');
    const width = Number.parseInt(upstream.headers.get('x-image-width') ?? '', 10);
    const height = Number.parseInt(upstream.headers.get('x-image-height') ?? '', 10);

    if (format !== 'rgb565be' || !Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) {
      res.status(502).send('invalid upstream image headers');
      return;
    }

    const rgb565 = Buffer.from(await upstream.arrayBuffer());
    const expectedLength = width * height * 2;
    if (rgb565.length !== expectedLength) {
      res.status(502).send(`invalid upstream payload length ${rgb565.length}, expected ${expectedLength}`);
      return;
    }

    const png = rgb565ToPng(rgb565, width, height);
    res.status(200);
    res.setHeader('Content-Type', 'image/png');
    res.setHeader('Content-Length', String(png.length));
    res.send(png);
  } catch (err) {
    console.error('failed to serve /info-panel.png', err);
    res.status(500).send('failed to generate info-panel.png');
  }
});

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
