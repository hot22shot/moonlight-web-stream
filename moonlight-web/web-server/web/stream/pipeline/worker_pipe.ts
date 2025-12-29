import { ExecutionEnvironment } from "../index.js";
import { Logger } from "../log.js";
import { VideoRendererSetup } from "../video/index.js";
import { Pipe, PipeInfo, Pipeline, pipelineToString, PipeStatic } from "./index.js";
import { addPipePassthrough } from "./pipes.js";
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

export interface WorkerReceiver extends Pipe {
    onWorkerMessage(message: WorkerMessage): void
}

export class WorkerPipe implements WorkerReceiver {
    static async getInfo(): Promise<PipeInfo> {
        return {
            // TODO: check in the actual worker for support
            executionEnvironment: await checkExecutionEnvironment("Worker")
        }
    }

    readonly implementationName: string

    private logger: Logger | null

    private worker: Worker
    private base: WorkerReceiver
    private pipeline: Pipeline

    constructor(base: WorkerReceiver, pipeline: Pipeline, logger?: Logger) {
        this.implementationName = `worker_pipe [${pipelineToString(pipeline)}] -> ${base.implementationName}`
        this.logger = logger ?? null

        // TODO: check that the pipeline starts with output and ends with input
        this.base = base
        this.pipeline = pipeline

        const worker = createPipelineWorker()
        if (!worker) {
            throw "Failed to create worker pipeline: Workers not supported!"
        }
        this.worker = worker

        this.worker.onmessage = this.onReceiveWorkerMessage.bind(this)

        addPipePassthrough(this)
    }

    onWorkerMessage(input: WorkerMessage): void {
        const message: ToWorkerMessage = { input }

        this.worker.postMessage(message)
    }

    private onReceiveWorkerMessage(event: MessageEvent) {
        const data: ToMainMessage = event.data

        if ("output" in data) {
            this.base.onWorkerMessage(data.output)
        } else if ("log" in data) {
            this.logger?.debug(data.log, data.info)
        }
    }

    setup(setup: VideoRendererSetup) {
        const message2: ToWorkerMessage = {
            createPipeline: this.pipeline
        }
        this.worker.postMessage(message2)

        this.onWorkerMessage({ videoSetup: setup })

        if ("setup" in this.base && typeof this.base.setup == "function") {
            return this.base.setup(...arguments)
        }
    }

    cleanup() {
        this.worker.terminate()

        if ("cleanup" in this.base && typeof this.base.cleanup == "function") {
            return this.base.cleanup(...arguments)
        }
    }

    getBase(): Pipe | null {
        return this.base
    }
}

export function workerPipe(name: string, pipeline: Pipeline): PipeStatic {
    // TODO: use name somehow
    class CustomWorkerPipe extends WorkerPipe {
        static readonly baseType = "workeroutput"
        static readonly type = "workerinput"

        constructor(base: any, logger?: Logger) {
            super(base, pipeline, logger)
        }
    }

    return CustomWorkerPipe
}