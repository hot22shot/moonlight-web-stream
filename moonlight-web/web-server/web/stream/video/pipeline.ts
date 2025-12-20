import { CanvasVideoRenderer } from "./canvas_element.js"
import { VideoElementRenderer } from "./video_element.js"
import { VideoMediaStreamTrackProcessorPipe } from "./media_stream_track_processor_pipe.js"
import { DataVideoRenderer, PacketVideoRenderer, TrackVideoRenderer, VideoRenderer } from "./index.js"
import { VideoDecoderPipe } from "./video_decoder_pipe.js"
import { DepacketizerVideoPipe } from "./depackitize_video_pipe.js"

type PipelineResult<T> = { videoRenderer: T, log: string, error: null } | { videoRenderer: null, log: string, error: string }

interface FinalVideoRenderer {
    new(): VideoRenderer

    readonly type: string
    isBrowserSupported(): boolean
}
const FINAL_VIDEO_RENDERER: Array<FinalVideoRenderer> = [
    VideoElementRenderer,
    CanvasVideoRenderer
]

interface VideoPipe {
    new(base: any): VideoRenderer

    readonly type: string
    isBrowserSupported(): boolean
}
const PIPE_TYPES: Array<string> = ["data", "videotrack", "videoframe"]
const VIDEO_PIPES: Record<string, VideoPipe> = {
    videotrack_to_videoframe: VideoMediaStreamTrackProcessorPipe
}

export type VideoPipelineOptions = {
    canvasRenderer: boolean
}

export function buildVideoPipeline(type: "videotrack", settings: VideoPipelineOptions): PipelineResult<TrackVideoRenderer>
export function buildVideoPipeline(type: "data", settings: VideoPipelineOptions): PipelineResult<PacketVideoRenderer>

export function buildVideoPipeline(type: string, settings: VideoPipelineOptions): PipelineResult<VideoRenderer> {
    let log = `Building video pipeline with output "${type}"`

    // Forced renderer
    if (settings.canvasRenderer) {
        if (type == "videotrack" && VideoMediaStreamTrackProcessorPipe.isBrowserSupported() && CanvasVideoRenderer.isBrowserSupported()) {
            const videoRenderer = new VideoMediaStreamTrackProcessorPipe(new CanvasVideoRenderer())

            return { videoRenderer, log, error: null }
        } else {
            throw "Failed to create video canvas renderer because it is not supported this this browser"
        }
    }

    if (type == "data") {
        if (VideoDecoderPipe.isBrowserSupported() && CanvasVideoRenderer.isBrowserSupported()) {
            const videoRenderer = new DepacketizerVideoPipe(new VideoDecoderPipe(new CanvasVideoRenderer()))

            return { videoRenderer, log, error: null }
        }
    }

    // TODO dynamically create pipelines based on browser support

    const directVideoRenderers = FINAL_VIDEO_RENDERER.filter(entry => entry.type == type && entry.isBrowserSupported())
    if (directVideoRenderers.length >= 1) {
        const videoRenderer = new directVideoRenderers[0]
        return { videoRenderer, log, error: null }
    }

    return { videoRenderer: null, log, error: "No supported video renderer found!" }
}