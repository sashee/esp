import {DatabaseSync} from "node:sqlite";
import path from "node:path";
import express from "express";
const app = express();
const port = 8080;

const database = new DatabaseSync(path.join(process.env.HOME, "monitoring-data", "data.db"));

const readAll = database.prepare("SELECT json_extract(value, '$.inverter.qpigs.ac_output_voltage') FROM data");

app.get('/', (req, res) => {
	console.log(readAll.all());
  res.send('Hello World!')
})

app.listen(port, () => {
  console.log(`Example app listening on port ${port}`)
})

