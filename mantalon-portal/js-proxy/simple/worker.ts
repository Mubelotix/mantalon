import { fromFakeUrl } from "../location";

export function setupWorkers() {
    const OriginalWorker = globalThis.Worker;

    globalThis.Worker = function (scriptURL: string | URL, options?: WorkerOptions): Worker {
        const realUrl = fromFakeUrl(scriptURL.toString(), location.protocol, location.host, location.port).href;
        console.warn(`Creating Worker with scriptURL: ${scriptURL} (rewritten to ${realUrl})`);

        return new OriginalWorker(realUrl, options);
    } as any as typeof Worker;
}
