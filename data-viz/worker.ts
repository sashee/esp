import {statements, database} from "./queries.ts";

export default (query) => {
  const statement = statements[query.sql];
  const params = JSON.parse(query.parameters);
  
  if (typeof statement === "function") {
  	const sql = statement(params);
  	console.log(sql, params);
  	return database.prepare(sql + ";").all(Object.fromEntries(Object.entries(params).filter(([k]) => !k.startsWith("_"))));
  } else {
  	return statement.all(params);
  }
}

