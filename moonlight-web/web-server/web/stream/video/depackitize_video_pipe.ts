import { ByteBuffer } from "../buffer.js";
import { Logger } from "../log.js";
import { Pipe } from "../pipeline/index.js";
import { addPipePassthrough, DataPipe } from "../pipeline/pipes.js";
import { allVideoCodecs } from "../video.js";
import { DataVideoRenderer, VideoRendererInfo, VideoRendererSetup } from "./index.js";

export class DepacketizeVideoPipe implements DataPipe {

    static readonly baseType = "videodata"
    static readonly type = "data"

    static async getInfo(): Promise<VideoRendererInfo> {
        // no link
        return {
            executionEnvironment: {
                main: true,
                worker: true
            },
            supportedCodecs: allVideoCodecs()
        }
    }

    readonly implementationName: string

    private base: DataVideoRenderer

    private frameDurationMicroseconds = 0
    private buffer = new ByteBuffer(5)

    constructor(base: DataVideoRenderer, logger?: Logger) {
        this.implementationName = `depacketize_video -> ${base.implementationName}`
        this.base = base

        addPipePassthrough(this)
    }

    submitPacket(buffer: ArrayBuffer) {
        const array = new Uint8Array(buffer)

        this.buffer.reset()

        this.buffer.putU8Array(array.slice(0, 5))

        this.buffer.flip()

        const frameType = this.buffer.getU8()
        const timestamp = this.buffer.getU32()

        this.base.submitDecodeUnit({
            type: frameType == 0 ? "delta" : "key",
            data: array.slice(5).buffer,
            durationMicroseconds: this.frameDurationMicroseconds,
            timestampMicroseconds: timestamp,
        })

        addPipePassthrough(this)
    }

    async setup(setup: VideoRendererSetup): Promise<void> {
        if ("setup" in this.base && typeof this.base.setup == "function") {
            await this.base.setup(setup)
        }
        this.frameDurationMicroseconds = 1000000 / setup.fps
    }

    getBase(): Pipe | null {
        return this.base
    }
}