import { Jimp, intToRGBA, rgbaToInt } from "jimp";

const image = await Jimp.read("inv2.jpg");

const isBlue = (color: RGBAColor) => {
	return color.b > color.g && color.b > color.r;
}

{
	const clone = image.clone();
	clone.scan((x, y) => {
		const color = intToRGBA(clone.getPixelColor(x,y));
		if (isBlue(color)) {
			clone.setPixelColor(rgbaToInt(0,0,255,255), x,y);
		}
	});

	await clone.write("1-bluepixels.png");
}

// find the largest blue region
const largestBlueArea = await (async () => {
	const clone = image.clone();
	const countAndEraseRegion = (x: number, y: number) => {
		let result = [];
		const coords = [{x, y}];
		while (coords.length !== 0) {
			const first = coords.pop()!;
			result.push(first);
			clone.setPixelColor(rgbaToInt(0,0,0,255), first.x,first.y);
			const moreBlue = [
				{x: first.x - 1, y: first.y - 1},
				{x: first.x - 1, y: first.y},
				{x: first.x - 1, y: first.y + 1},
				{x: first.x, y: first.y - 1},
				{x: first.x, y: first.y + 1},
				{x: first.x + 1, y: first.y - 1},
				{x: first.x + 1, y: first.y},
				{x: first.x + 1, y: first.y + 1},
			]
			.filter(({x, y}) => x >= 0 && x < clone.width && y >= 0 && y < clone.height)
			.filter(({x, y}) => isBlue(intToRGBA(clone.getPixelColor(x, y))));
			coords.push(...moreBlue);
		}
		return result;
	};

	let largest = undefined;
	clone.scan((x, y) => {
		const color = intToRGBA(clone.getPixelColor(x,y));
		if (isBlue(color)) {
			const res = countAndEraseRegion(x,y);
			if (!largest || (res.length > largest.length)) {
				largest = res;
			}
		}
	});

	{
		const clone = image.clone();
		clone.scan((x, y) => {
			clone.setPixelColor(rgbaToInt(0,0,0,255), x,y);
		});
		for (const {x,y} of largest) {
			clone.setPixelColor(rgbaToInt(0,0,255,255), x,y);
		}

		await clone.write("2-largest-bluepixels.png");
	}
	return largest;
})();

console.log(largestBlueArea)

{
	let extremes = {};
	for (const {x,y} of largestBlueArea) {
		extremes.left = Math.min(x, extremes.left ?? Number.MAX_SAFE_INTEGER);
		extremes.right = Math.max(x, extremes.right ?? 0);
		extremes.top = Math.min(y, extremes.top ?? Number.MAX_SAFE_INTEGER);
		extremes.bottom = Math.max(y, extremes.bottom ?? 0);
	}
	console.log(extremes);
	const clone = image.clone();
	clone.scan((x, y) => {
		clone.setPixelColor(rgbaToInt(0,0,0,0), x,y);
	});
	for (const {x,y} of largestBlueArea) {
		clone.setPixelColor(rgbaToInt(0,0,255,255), x,y);
	}
	clone.crop({x: extremes.left, y: extremes.top, w: extremes.right - extremes.left +1, h: extremes.bottom - extremes.top +1});

	// https://stackoverflow.com/a/37865332
	const pointInRectangle = (m: {x: number, y: number}, r: {A: {x: number, y: number}, B: {x: number, y: number}, C: {x: number, y: number}, D: {x: number, y: number}}) => {
		function vector(p1: {x: number, y: number}, p2: {x: number, y: number}) {
				return {
						x: (p2.x - p1.x),
						y: (p2.y - p1.y)
				};
		}

		function dot(u: {x: number, y: number}, v: {x: number, y: number}) {
				return u.x * v.x + u.y * v.y;
		}
    var AB = vector(r.A, r.B);
    var AM = vector(r.A, m);
    var BC = vector(r.B, r.C);
    var BM = vector(r.B, m);
    var dotABAM = dot(AB, AM);
    var dotABAB = dot(AB, AB);
    var dotBCBM = dot(BC, BM);
    var dotBCBC = dot(BC, BC);
    return 0 <= dotABAM && dotABAM <= dotABAB && 0 <= dotBCBM && dotBCBM <= dotBCBC;
	}
	let A = {x: 0, y: 0};
	let B = {x: clone.width - 1, y: 0};
	let C = {x: 0, y: clone.height - 1};
	let D = {x: clone.width - 1, y: clone.height - 1};
	const calculateScore = (A: {x: number, y: number}, B: {x: number, y: number}, C: {x: number, y: number}, D: {x: number, y: number}) => {
		let blueIn = 0;
		let notBlueIn = 0;
		clone.scan((x, y) => {
			if (pointInRectangle({x,y}, {A,B,C,D})) {
				if (intToRGBA(clone.getPixelColor(x, y)).b === 255) {
					blueIn++;
				}else {
					notBlueIn++;
				}
			}
		});
		return {blueIn, notBlueIn};
	}
	for (let i = 0; i < 100; i++) {
		clone.setPixelColor(rgbaToInt(255,0,0,255), A.x,A.y);
		clone.setPixelColor(rgbaToInt(255,0,0,255), B.x,B.y);
		clone.setPixelColor(rgbaToInt(255,0,0,255), C.x,C.y);
		clone.setPixelColor(rgbaToInt(255,0,0,255), D.x,D.y);
		const possibleModifications = (x: number, y: number) => {
			return [
				{x: x - 1, y: y - 1},
				{x: x - 1, y: y},
				{x: x - 1, y: y + 1},
				{x: x, y: y - 1},
				{x: x, y: y + 1},
				{x: x + 1, y: y - 1},
				{x: x + 1, y: y},
				{x: x + 1, y: y + 1},
			]
			.filter(({x, y}) => x >= 0 && x < clone.width && y >= 0 && y < clone.height)
		}
		const possibleRects = [
			...possibleModifications(A.x, A.y).map(({x,y}) => ({A:{x,y}, B, C, D})),
			...possibleModifications(B.x, B.y).map(({x,y}) => ({B:{x,y}, A, C, D})),
			...possibleModifications(C.x, C.y).map(({x,y}) => ({C:{x,y}, B, A, D})),
			...possibleModifications(D.x, D.y).map(({x,y}) => ({D:{x,y}, B, C, A})),
		];
		const best = possibleRects.reduce((memo, rect) => {
			const score = calculateScore(rect.A, rect.B, rect.C, rect.D);
			if (!memo) {
				return {rect, score};
			}else {
				const calc = (score) => score.blueIn *10- score.notBlueIn;
				if (calc(score) > calc(memo.score)) {
					return {rect, score};
				}else {
					return memo;
				}
			}
		}, undefined);
		A = best.rect.A;
		B = best.rect.B;
		C = best.rect.C;
		D = best.rect.D;
		//console.log(best.score)
	}

	await clone.write("3-cropped.png");
}
{
	let extremes = {};
	for (const {x,y} of largestBlueArea) {
		extremes.left = Math.min(x, extremes.left ?? Number.MAX_SAFE_INTEGER);
		extremes.right = Math.max(x, extremes.right ?? 0);
		extremes.top = Math.min(y, extremes.top ?? Number.MAX_SAFE_INTEGER);
		extremes.bottom = Math.max(y, extremes.bottom ?? 0);
	}
	console.log(extremes);
	const clone = image.clone();
	clone.scan((x, y) => {
		clone.setPixelColor(rgbaToInt(0,0,0,0), x,y);
	});
	for (const {x,y} of largestBlueArea) {
		clone.setPixelColor(rgbaToInt(0,0,255,255), x,y);
	}
	clone.crop({x: extremes.left, y: extremes.top, w: extremes.right - extremes.left +1, h: extremes.bottom - extremes.top +1});
	const best = Array(360).fill(null).map((_e, i) => i).reduce((memo, deg) => {
		const img = clone.clone().rotate(deg).autocrop();
		if (!memo) {
			return {deg, size: img.width * img.height};
		}else {
			if (img.width * img.height < memo.size) {
				return {deg, size: img.width * img.height};
			}else {
				return memo;
			}
		}
	}, undefined);
	console.log(best)
	await clone.clone().rotate(best.deg).autocrop().write("4-rotated.png");
}
