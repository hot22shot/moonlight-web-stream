import { Component } from "../../component/index.js"
import { StreamSupportedVideoFormats } from "../../api_bindings.js"

export type VideoRenderer = (DataVideoRenderer | TrackVideoRenderer)

export type VideoRendererSetup = {
    format: keyof typeof StreamSupportedVideoFormats,
    width: number
    height: number
    fps: number
}

interface VideoRendererBase extends Component {
    readonly implementationName: string
    readonly type: string

    setup(setup: VideoRendererSetup): void
    cleanup(): void

    onUserInteraction(): void
    getStreamRect(): DOMRect
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

export interface TrackVideoRenderer extends VideoRendererBase {
    readonly type: "stream"

    setTrack(track: MediaStreamTrack): void
}

export type VideoDecodeUnit = {
    type: "key" | "delta"
    timestampMicroseconds: number
    durationMicroseconds: number
    data: ArrayBuffer
}

export interface DataVideoRenderer extends VideoRendererBase {
    readonly type: "moonlightdata"

    // Data like https://github.com/moonlight-stream/moonlight-common-c/blob/b126e481a195fdc7152d211def17190e3434bcce/src/Limelight.h#L298
    submitDecodeUnit(unit: VideoDecodeUnit): void
}
