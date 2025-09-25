#!/usr/bin/env node
/**
 * Test harness for applyJsProxyOnJs
 *
 * Usage:
 *   node test-apply-js-proxy.js
 */

import fs from "fs/promises";
import path from "path";
import { applyJsProxyOnJs } from "../dist/js-rewriter.js";

/**
 * Array of test cases.
 * Each test case should have:
 *   - name: a string describing the test
 *   - input: the JS source code string
 *   - expected: the expected transformed JS string
 */
const testCases = [
    {
        name: "Simple function rewrite",
        input: `
        if (window.location.href !== "https://example.com") {
            window.location.replace("https://example.com");
        }
        `,
        expected: `
        if (mantalon(mantalon(mantalon(window).location).href) !== "https://example.com") {
            mantalon(mantalon(mantalon(window).location).replace).call(mantalon(mantalon(window).location), "https://example.com");
        }`,
    },
    {
        name: "Nested member expressions",
        input: `
        console.log(window.document.body.style.backgroundColor);
        window.document.body.style.color = "red";
        `,
        expected: `
        mantalon(mantalon(console).log).call(mantalon(console), mantalon(
                mantalon(mantalon(mantalon(mantalon(window).document).body).style).backgroundColor
        ));
        mantalon(mantalon(mantalon(mantalon(window).document).body).style).color = "red";
        `,
    },
    {
        name: "Call expressions on members",
        input: `
        window.alert("Hello, world!");
        document.querySelector("#app").innerHTML = "Test";
        `,
        expected: `
        mantalon(mantalon(window).alert).call(mantalon(window), "Hello, world!");
        mantalon(mantalon(document).querySelector).call(mantalon(document), "#app").innerHTML = "Test";
        `,
    },
    {
        name: "Assignments",
        input: `
        window.document.title = "My Page";
        location.hash = "#section1";
        `,
        expected: `
        mantalon(mantalon(window).document).title = "My Page";
        mantalon(location).hash = "#section1";
        `,
    },
    {
        name: "Object destructuring (should not wrap variable names)",
        input: `
        const { body, title } = document;
        `,
        expected: `
        const { body, title } = mantalon(document);
        `,
    },
    {
        name: "Arrow function and function declarations (parameters should not be wrapped)",
        input: `
        const update = (doc) => {
            console.log(doc.body);
        };

        function logWindow(win) {
            console.log(win.location);
        }
        `,
        expected: `
        const update = (doc) => {
            mantalon(mantalon(console).log).call(mantalon(console), mantalon(mantalon(doc).body));
        };

        function logWindow(win) {
            mantalon(mantalon(console).log).call(mantalon(console), mantalon(mantalon(win).location));
        }
        `,
    },
    {
        name: "Array accesses (computed properties)",
        input: `
        const frames = window.frames;
        const firstFrame = frames[0].document;
        `,
        expected: `
        const frames = mantalon(mantalon(window).frames);
        const firstFrame = mantalon(mantalon(mantalon(frames)[0]).document);
        `,
    },
    {
        name: "Property access on a computed object",
        input: `
        const key = "href";
        window.location[key] = "https://changed.com";
        `,
        expected: `
        const key = "href";
        mantalon(mantalon(window).location)[key] = "https://changed.com";
        `,
    },
    {
        name: "Chained calls with arguments",
        input: `
        window.location.assign("https://another.com");    
        `,
        expected: `
        mantalon(mantalon(mantalon(window).location).assign).call(mantalon(mantalon(window).location), "https://another.com");    
        `,
    },
    {
        name: "Member expression with nested computed property",
        input: `
        const styleKey = "color";
        document.body.style[styleKey] = "blue";    
        `,
        expected: `
        const styleKey = "color";
        mantalon(mantalon(mantalon(document).body).style)[styleKey] = "blue";
        `,
    },
    {
        name: "Mixing literals, computed, and normal properties",
        input: `
        window["document"].title = "Title via bracket";
        window.document["body"].style.backgroundColor = "green";    
        `,
        expected: `
        mantalon(mantalon(window)["document"]).title = "Title via bracket";
        mantalon(mantalon(mantalon(mantalon(window).document)["body"]).style).backgroundColor = "green";
        `,
    },
    {
        name: "Function call on a wrapped property",
        input: `
        document.body.appendChild(document.createElement("div"));    
        `,
        expected: `
        mantalon(mantalon(mantalon(document).body).appendChild).call(mantalon(mantalon(document).body), mantalon(mantalon(document).createElement).call(mantalon(document), "div"));    
        `,
    },
    {
        name: "Combination of everything",
        input: `
        if (window.location.protocol === "http:" && window.location.hostname !== "example.com") {
            window.location.href = "https://example.com" + window.location.pathname + window.location.search + window.location.hash;
        }`,
        expected: `
        if (mantalon(mantalon(mantalon(window).location).protocol) === "http:" && mantalon(mantalon(mantalon(window).location).hostname) !== "example.com") {
            mantalon(mantalon(window).location).href = "https://example.com" + mantalon(mantalon(mantalon(window).location).pathname) + mantalon(mantalon(mantalon(window).location).search) + mantalon(mantalon(mantalon(window).location).hash);
        }
        `,
    },
    {
        name: "Self webpack chunk",
        input: `
        self.webpackChunk_N_E    
        `,
        expected: `mantalon(mantalon(self).webpackChunk_N_E)`,
    },
    {
        name: "Inline function",
        input: `
        a = function(a, b) {
            return a + b;
        }
        `,
        expected: `
        a = function(a, b) {
            return mantalon(a) + mantalon(b);
        }
        `,
    },
    {
        name: "Try catch block",
        input: `
        try {
            console.log("whatever");
        } catch (e) {
            console.error(e);
        }
        `,
        expected: `
        try {
            mantalon(mantalon(console).log).call(mantalon(console), "whatever");
        } catch (e) {
            mantalon(mantalon(console).error).call(mantalon(console), mantalon(e));
        }
        `,
    }
];

function normalize(str) {
    return str.replace(/\s+$/gm, "").trim(); // trims trailing spaces per line and overall
}

/**
 * Run all test cases
 */
async function runTests() {
    let passed = 0;

    for (const { name, input, expected } of testCases) {
        try {
            const output = applyJsProxyOnJs(input);
            if (normalize(output) === normalize(expected)) {
                console.log(`✅ [PASS] ${name}`);
                passed++;
            } else {
                console.error(`❌ [FAIL] ${name}`);
                console.error("Input:\n", input);
                console.error("Expected:\n", expected);
                console.error("Got:\n", output);
                console.error("-----");
            }
        } catch (err) {
            console.error(`❌ [ERROR] ${name}`);
            console.error(err);
        }
    }

    console.log(`\n${passed}/${testCases.length} tests passed.`);
    process.exit(passed === testCases.length ? 0 : 1);
}

runTests();
