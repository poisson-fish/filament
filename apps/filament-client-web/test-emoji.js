import data from "@emoji-mart/data/sets/14/twitter.json" assert { type: "json" };
console.log(data.emojis['joy']);
console.log(Object.keys(data.emojis).find(k => k === 'joy' || data.emojis[k].shortcodes?.includes(':joy:')));
