import { Logger } from "../log.js";
import { FrameVideoRenderer, VideoRendererSetup } from "../video/index.js";
import { Pipe } from "./index.js";
import { addPipePassthrough, DataPipe } from "./pipes.js";
import { WorkerPipe, WorkerReceiver } from "./worker_pipe.js";
import { WorkerMessage } from "./worker_types.js";

export class WorkerReceiverPipe implements WorkerReceiver, DataPipe, FrameVideoRenderer {
    static readonly type = "workerinput"

    readonly implementationName: string

    private base: Pipe

    constructor(base: Pipe) {
        this.implementationName = `worker_recv -> ${base.implementationName}`

        this.base = base

        addPipePassthrough(this, ["setup", "cleanup", "submitFrame", "submitPacket"])
    }

    onWorkerMessage(message: WorkerMessage): void {
        if ("call" in message && message.call == "cleanup") {
            this.cleanup()
        } else if ("videoSetup" in message) {
            this.setup(message.videoSetup)
        } else if ("videoFrame" in message) {
            this.submitFrame(message.videoFrame)
        } else if ("data" in message) {
            this.submitPacket(message.data)
        }
    }

    getBase(): Pipe {
        return this.base
    }

    // -- Only definition look addPipePassthrough
    setup(_setup: VideoRendererSetup): void { }
    cleanup(): void { }
    submitFrame(_frame: VideoFrame): void { }
    submitPacket(_buffer: ArrayBuffer): void { }
}

export class WorkerSenderPipe implements DataPipe, FrameVideoRenderer {
    readonly implementationName: string

    private base: WorkerPipe

    constructor(base: WorkerPipe, _logger?: Logger) {
        this.implementationName = `worker_send -> ${base.implementationName}`
        this.base = base

        addPipePassthrough(this)
    }

    getBase(): WorkerPipe {
        return this.base
    }

    submitFrame(videoFrame: VideoFrame): void {
        this.getBase()?.onWorkerMessage({ videoFrame })
    }
    submitPacket(data: ArrayBuffer): void {
        this.getBase()?.onWorkerMessage({ data })
    }
}
