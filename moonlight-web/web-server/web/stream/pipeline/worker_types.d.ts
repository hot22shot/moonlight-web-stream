import { LogMessageInfo } from "../log.js"
import { VideoRendererSetup } from "../video/index.js"
import { Pipeline } from "./index.js"

export type ToWorkerMessage =
    { checkSupport: { className: string } } |
    { createPipeline: Pipeline } |
    { input: WorkerMessage }

export type WorkerMessage =
    { call: "cleanup" } |
    { videoSetup: VideoRendererSetup } |
    // VideoFrame is a transferable object
    { videoFrame: VideoFrame } |
    { data: ArrayBuffer }

export type ToMainMessage =
    { checkSupport: { supported: boolean } } |
    { log: string, info: LogMessageInfo } |
    { output: WorkerMessage }
