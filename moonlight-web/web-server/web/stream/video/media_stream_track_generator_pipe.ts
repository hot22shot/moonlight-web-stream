import { Pipe } from "../pipeline/index.js";
import { addPipePassthrough } from "../pipeline/pipes.js";
import { checkExecutionEnvironment } from "../pipeline/worker_pipe.js";
import { allVideoCodecs } from "../video.js";
import { FrameVideoRenderer, TrackVideoRenderer, VideoRendererInfo } from "./index.js";

export class VideoMediaStreamTrackGeneratorPipe implements FrameVideoRenderer {

    static readonly baseType = "videotrack"
    static readonly type = "videoframe"

    static async getInfo(): Promise<VideoRendererInfo> {
        // https://developer.mozilla.org/en-US/docs/Web/API/MediaStreamTrackGenerator
        return {
            executionEnvironment: await checkExecutionEnvironment("MediaStreamTrackGenerator"),
            supportedCodecs: allVideoCodecs()
        }
    }

    readonly implementationName: string

    private base: TrackVideoRenderer

    private trackGenerator: MediaStreamTrackGenerator
    private writer: WritableStreamDefaultWriter<VideoFrame>

    constructor(base: TrackVideoRenderer) {
        this.implementationName = `video_media_stream_track_generator -> ${base.implementationName}`
        this.base = base

        this.trackGenerator = new MediaStreamTrackGenerator({ kind: "video" })
        this.writer = this.trackGenerator.writable.getWriter()

        addPipePassthrough(this)
    }

    private isFirstSample = true

    submitFrame(frame: VideoFrame): void {
        if (this.isFirstSample) {
            this.isFirstSample = false

            this.base.setTrack(this.trackGenerator)
        }
        this.writer.write(frame)
    }

    getBase(): Pipe | null {
        return this.base
    }
}