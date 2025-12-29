import { LogMessageType } from "../../api_bindings.js"
import { Logger } from "../log.js"
import { buildPipeline, Pipe } from "./index.js"
import { WorkerReceiver } from "./worker_pipe.js"
import { ToMainMessage, ToWorkerMessage, WorkerMessage } from "./worker_types.js"

// Configure logger
const logger = new Logger()

function onLog(text: string, type: LogMessageType | null) {
    const message: ToMainMessage = {
        log: text,
        info: { type: type ?? undefined }
    }

    postMessage(message)
}

logger?.addInfoListener(onLog)

let currentPipeline: WorkerReceiver | null = null

class WorkerMessageSender implements WorkerReceiver {
    static readonly type: string = "workerinput"

    readonly implementationName: string = "worker_output"

    constructor(logger?: Logger) {
    }

    onWorkerMessage(output: WorkerMessage): void {
        const message: ToMainMessage = { output }

        postMessage(message)
    }

    getBase(): Pipe | null {
        return null
    }
}

function onMessage(message: ToWorkerMessage) {
    if ("checkSupport" in message) {
        const className = message.checkSupport.className

        const supported = className in self

        const response: ToMainMessage = {
            checkSupport: { supported }
        }
        postMessage(response)
    } else if ("createPipeline" in message) {
        const pipeline = message.createPipeline

        // TODO: create logger
        const newPipeline = buildPipeline(WorkerMessageSender, pipeline, logger)
        if (newPipeline && "onWorkerMessage" in newPipeline && typeof newPipeline.onWorkerMessage == "function") {
            currentPipeline = newPipeline as WorkerReceiver
        } else {
            // TODO: error
            throw "failed to create pipeline"
        }
    } else if ("input" in message) {
        if (currentPipeline) {
            currentPipeline.onWorkerMessage(message.input)
        } else {
            // TODO: error
            throw "no pipeline"
        }
    }
}

onmessage = (event) => {
    const message = event.data as ToWorkerMessage
    onMessage(message)
}