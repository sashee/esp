const echarts = await import("https://esm.sh/echarts@6.0.0");

await Promise.all([...document.querySelectorAll(".chart")].map(async (e) => {
	const observer = new IntersectionObserver((entries, observer) => {
		entries.filter((entry) => entry.isIntersecting).forEach(async () => {
			observer.unobserve(e);
			const sql = e.dataset.sql;
			console.log(sql)
			console.log((e.dataset))
			const timePeriod = {
				from: new Date(new Date().getTime() - 1000*60*60*24*2).getTime(),
				to: new Date().getTime(),
			};
			const datas = await Promise.all(Object.entries(e.dataset).filter(([n]) => n.startsWith("series-")).map(async ([, series]) => {
				const {type, params, ...rest} = JSON.parse(series);
				const res = await fetch(`/sql?sql=${encodeURIComponent(type)}&parameters=${encodeURIComponent(JSON.stringify({...params, ...timePeriod}))}`);
				if (!res.ok) {
					console.error(res);
					throw new Error("fetch failed");
				}
				const data = await res.json();
				return {data: new Map(data.map(({timestamp, data}) => [timestamp, data])), ...rest};
			}));

			const uniqueTimestamps = new Set(...datas.map((d) => d.data.keys()));
			const mergedData = [...uniqueTimestamps.entries().map(([timestamp]) => {
				return [
					new Date(timestamp).toISOString(),
					...datas.map(({data}) => data.get(timestamp) ?? null),
				];
			})];

			var myChart = echarts.init(e);
myChart.setOption({
	dataset: {
		source: mergedData,
		dimensions: ['timestamp', ...datas.map((_d, i) => `data_${i}`)],
	},
	animation: false,
	grid: {
    left: 0,
    right: 0,
    top: 50,
    bottom: 0,
  },
	title: { text: e.dataset.title},
	tooltip: {
		trigger: 'axis',
		axisPointer: {
			type: 'line',
			lineStyle: {
				color: '#aaa',
				width: 1
			}
		}
	},
  xAxis: {
    type: 'time',
		min: new Date(timePeriod.from),
		max: new Date(timePeriod.to),
  },
  yAxis: {
    type: 'value'
  },
	series: datas.map(({title, lineStyle}, i) => {
		return {
			name: title,
			type: 'line',
			encode: {
				x: "timestamp",
				y: `data_${i}`
			},
			...(lineStyle ? {lineStyle} : {}),
			symbol: "none",
		}
	}),
});
					console.log("RENDERED")
		});
	}, {
		rootMargin: "0px",
		scrollMargin: "0px",
		threshold: 0,
	});
	observer.observe(e);
}));
