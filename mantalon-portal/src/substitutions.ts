import { loadRessource, Substitution, SubstitutionConfig } from "./manifest";
import { manifest, orDefault } from "./sw";

export async function applySubstitutions(response: Response | string, url: URL, contentType: string): Promise<string | undefined> {
    if (response instanceof Response && response.body === null) {
        return undefined;
    }

    let substitutionsConfig = manifest.substitutions?.find((conf) => conf.test(url, contentType));
    let contentScriptsConfig = manifest.content_scripts?.find((conf) => conf.test(url, contentType));

    if (!substitutionsConfig && !contentScriptsConfig) {
        return undefined;
    }

    if (!substitutionsConfig) {
        substitutionsConfig = new SubstitutionConfig({
            matches: ["https://example.com"],
            substitutions: []
        });
    }

    if (substitutionsConfig.substitutions.length == 0
        && orDefault(contentScriptsConfig?.js?.length, 0) == 0
        && orDefault(contentScriptsConfig?.css?.length, 0) == 0)
    {
        return undefined;
    }

    // Start loading body while we load ressources
    let bodyPromise: Promise<string>;
    if (response instanceof Response) {
        bodyPromise = response.text();
    } else {
        bodyPromise = Promise.resolve(response);
    }

    if (contentScriptsConfig) {
        for (let css of contentScriptsConfig.css || []) {
            let contentResponse = await loadRessource(css);
            let content = await contentResponse?.text();
            if (!content) {
                console.error(`Couldn't inject css due to data being unavailable: ${css}`);
                continue;
            }
            substitutionsConfig.substitutions.push(new Substitution ({
                pattern: "<head>",
                replacement: `<style>${content}</style>`,
                insert: true,
                once: true,
                prevent_duplicates: true
            }));
        }
        for (let js of contentScriptsConfig.js || []) {
            let contentResponse = await loadRessource(js);
            let content = await contentResponse?.text();
            if (!content) {
                console.error(`Couldn't inject js due to data being unavailable: ${js}`);
                continue;
            }
            substitutionsConfig.substitutions.push(new Substitution ({
                pattern: "<head>",
                replacement: `<script>${content}</script>`,
                insert: true,
                once: true,
                prevent_duplicates: true
            }));
        }
    }

    let body = await bodyPromise;
    for (let substitution of substitutionsConfig.substitutions) {
        let pattern = substitution.pattern;
        let replacement = substitution.replacement;

        if (orDefault(substitution.prevent_duplicates, true)) {
            if (body.includes(replacement)) {
                continue;
            }
        }

        if (orDefault(substitution.insert, false)) {
            replacement = pattern + replacement;
        }

        if (orDefault(substitution.once, false)) {
            body = body.replace(pattern, replacement);
        } else {
            body = body.replaceAll(pattern, replacement);
        }
    }
    return body;
}
