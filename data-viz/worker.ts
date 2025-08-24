import {statements} from "./queries.ts";

export default (query) => {
	console.log(query)
  return statements[query.sql].all(JSON.parse(query.parameters));
}
