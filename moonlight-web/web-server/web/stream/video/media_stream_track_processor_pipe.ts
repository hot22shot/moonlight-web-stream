import { Pipe } from "../pipeline/index.js";
import { addPipePassthrough } from "../pipeline/pipes.js";
import { checkExecutionEnvironment } from "../pipeline/worker_pipe.js";
import { allVideoCodecs } from "../video.js";
import { FrameVideoRenderer, TrackVideoRenderer, VideoRendererInfo, VideoRendererSetup } from "./index.js";

function wait(time: number): Promise<void> {
    return new Promise((resolve, _reject) => {
        setTimeout(resolve, time)
    })
}

export class VideoMediaStreamTrackProcessorPipe implements TrackVideoRenderer {

    static readonly baseType = "videoframe"
    static readonly type = "videotrack"

    static async getInfo(): Promise<VideoRendererInfo> {
        // https://developer.mozilla.org/en-US/docs/Web/API/MediaStreamTrackProcessor
        return {
            executionEnvironment: await checkExecutionEnvironment("MediaStreamTrackProcessor"),
            supportedCodecs: allVideoCodecs()
        }
    }

    readonly implementationName: string

    private running: boolean = false
    private newProcessor: boolean = false
    private trackProcessor: MediaStreamTrackProcessor | null = null

    private base: FrameVideoRenderer

    constructor(base: FrameVideoRenderer) {
        this.implementationName = `media_stream_track_processor -> ${base.implementationName}`
        this.base = base

        addPipePassthrough(this)
    }

    setTrack(track: MediaStreamTrack): void {
        this.trackProcessor = new MediaStreamTrackProcessor({ track })
        this.newProcessor = true
    }

    private async readTrack() {
        let reader: ReadableStreamDefaultReader<VideoFrame> | null = null

        while (this.running) {
            if (!reader || this.newProcessor) {
                this.newProcessor = false

                if (this.trackProcessor?.readable.locked) {
                    // Shouldn't happen
                    throw "Canvas video track processor is locked"
                }

                const newReader = this.trackProcessor?.readable.getReader()
                if (newReader) {
                    reader = newReader
                }
                await wait(100)
                continue
            }

            // TODO: byob?
            const { done, value } = await reader.read()
            if (done) {
                console.error("Track Processor is done!")
                return
            }

            this.base.submitFrame(value)
        }
    }

    async setup(setup: VideoRendererSetup): Promise<void> {
        this.running = true
        this.readTrack()

        if ("setup" in this.base && typeof this.base.setup == "function") {
            await this.base.setup(setup)
        }
    }
    cleanup(): void {
        this.running = false
        try {
            if (this.trackProcessor) {
                this.trackProcessor.readable.cancel()
            }
        } catch (e) {
            console.error(e)
        }
        this.trackProcessor = null

        if ("cleanup" in this.base && typeof this.base.cleanup == "function") {
            this.base.cleanup()
        }
    }

    getBase(): Pipe | null {
        return this.base
    }
}