import { Component } from "../../component/index.js"
import { StreamSupportedVideoFormats } from "../../api_bindings.js"
import { ExecutionEnvironment } from "../index.js"
import { ToMainMessage, ToWorkerMessage } from "../pipeline/worker_types.js"

export type VideoRendererInfo = {
    executionEnvironment: ExecutionEnvironment
}

export type VideoRendererSetup = {
    format: keyof typeof StreamSupportedVideoFormats,
    width: number
    height: number
    fps: number
}

export abstract class VideoRenderer implements Component {
    readonly implementationName: string

    constructor(implementationName: string) {
        this.implementationName = implementationName
    }

    /// Returns the success
    abstract setup(setup: VideoRendererSetup): void
    abstract cleanup(): void

    abstract onUserInteraction(): void
    abstract getStreamRect(): DOMRect

    // Don't work inside a worker
    abstract mount(parent: HTMLElement): void
    // Don't work inside a worker
    abstract unmount(parent: HTMLElement): void
}

export function getStreamRectCorrected(boundingRect: DOMRect, videoSize: [number, number]): DOMRect {
    const videoAspect = videoSize[0] / videoSize[1]

    const boundingRectAspect = boundingRect.width / boundingRect.height

    let x = boundingRect.x
    let y = boundingRect.y
    let videoMultiplier
    if (boundingRectAspect > videoAspect) {
        // How much is the video scaled up
        videoMultiplier = boundingRect.height / videoSize[1]

        // Note: Both in boundingRect / page scale
        const boundingRectHalfWidth = boundingRect.width / 2
        const videoHalfWidth = videoSize[0] * videoMultiplier / 2

        x += boundingRectHalfWidth - videoHalfWidth
    } else {
        // Same as above but inverted
        videoMultiplier = boundingRect.width / videoSize[0]

        const boundingRectHalfHeight = boundingRect.height / 2
        const videoHalfHeight = videoSize[1] * videoMultiplier / 2

        y += boundingRectHalfHeight - videoHalfHeight
    }

    return new DOMRect(
        x,
        y,
        videoSize[0] * videoMultiplier,
        videoSize[1] * videoMultiplier
    )
}

export abstract class TrackVideoRenderer extends VideoRenderer {
    static readonly type: string = "videotrack"

    abstract setTrack(track: MediaStreamTrack): void
}

export abstract class FrameVideoRenderer extends VideoRenderer {
    static readonly type: string = "videoframe"

    /// Submits a frame. This renderer now "owns" the frame and needs to clean it up via close.
    abstract submitFrame(frame: VideoFrame): void
}

export type VideoDecodeUnit = {
    type: "key" | "delta"
    timestampMicroseconds: number
    durationMicroseconds: number
    /*
      Contains the data for one frame:
      - H264:
        - keyframe: Must contain sps,pps,idr(one or multiple)
        - delta: Must contain the whole frame(one or multiple CodecSliceNonIdr's)
    */
    data: ArrayBuffer
}

export abstract class DataVideoRenderer extends VideoRenderer {
    static readonly type: string = "videodata"

    /// Data like https://github.com/moonlight-stream/moonlight-common-c/blob/b126e481a195fdc7152d211def17190e3434bcce/src/Limelight.h#L298
    abstract submitDecodeUnit(unit: VideoDecodeUnit): void
}

export abstract class PacketVideoRenderer extends VideoRenderer {
    static readonly type: string = "data"

    abstract submitPacket(buffer: ArrayBuffer): void
}

export function createVideoWorker(): Worker {
    return new Worker(new URL("worker.js", import.meta.url), { type: "module" })
}

function checkWorkerSupport(className: string): Promise<boolean> {

    return new Promise((resolve, reject) => {
        const worker = createVideoWorker()

        worker.onerror = reject
        worker.onmessageerror = reject

        worker.onmessage = (message) => {
            const data = message.data as ToMainMessage

            resolve(data.checkSupport.supported)
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
