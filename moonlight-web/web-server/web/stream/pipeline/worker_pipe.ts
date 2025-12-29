import { ExecutionEnvironment } from "../index.js";
import { Logger } from "../log.js";
import { Pipe, Pipeline, pipelineToString, PipeStatic } from "./index.js";
import { ToMainMessage, ToWorkerMessage, WorkerMessage } from "./worker_types.js";

export function createPipelineWorker(): Worker | null {
    if (!("Worker" in window)) {
        return null
    }

    return new Worker(new URL("worker.js", import.meta.url), { type: "module" })
}

function checkWorkerSupport(className: string): Promise<boolean> {
    return new Promise((resolve, reject) => {
        const worker = createPipelineWorker()
        if (!worker) {
            resolve(false)
            return
        }

        worker.onerror = reject
        worker.onmessageerror = reject

        worker.onmessage = (message) => {
            const data = message.data as ToMainMessage

            if ("checkSupport" in data) {
                resolve(data.checkSupport.supported)

                worker.terminate()
            } else {
                throw `Received invalid message whilst checking support of a worker ${JSON.stringify(data)}`
            }
        }

        const request: ToWorkerMessage = {
            checkSupport: { className }
        }
        worker.postMessage(request)
    });
}

export async function checkExecutionEnvironment(className: string): Promise<ExecutionEnvironment> {
    return {
        main: className in window,
        worker: await checkWorkerSupport(className),
    }
}

// Some components might have mounting / unmounting elements to / from the dom
// -> We also need to pass them through
export interface WorkerReceiver extends Pipe {
    onWorkerMessage(message: WorkerMessage): void
}

export class WorkerPipe implements WorkerReceiver {
    static readonly baseType = "workeroutput"
    static readonly type = "workerinput"

    readonly implementationName: string

    private logger: Logger | null

    private worker: Worker
    private base: WorkerReceiver

    constructor(base: WorkerReceiver, pipeline: Pipeline, logger?: Logger) {
        this.implementationName = `worker_pipe [${pipelineToString(pipeline)}] -> ${base.implementationName}`
        this.logger = logger ?? null

        // TODO: check that the pipeline starts with output and ends with input
        this.base = base

        const worker = createPipelineWorker()
        if (!worker) {
            throw "Failed to create worker pipeline: Workers not supported!"
        }
        this.worker = worker

        this.worker.onmessage = this.onReceiveWorkerMessage.bind(this)
    }

    onWorkerMessage(input: WorkerMessage): void {
        const message: ToWorkerMessage = { input }

        this.worker.postMessage(message)
    }

    private onReceiveWorkerMessage(event: MessageEvent) {
        const data: ToMainMessage = event.data

        if ("output" in data) {
            this.base.onWorkerMessage(data.output)
        }
    }

    cleanup(): void {
        this.worker.terminate()
    }

    getBase(): Pipe | null {
        return this.base
    }
}

export function workerPipe(pipeline: Pipeline): PipeStatic {
    class CustomWorkerPipe extends WorkerPipe {
        constructor(base: any, logger?: Logger) {
            super(base, pipeline, logger)
        }
    }

    return CustomWorkerPipe
}