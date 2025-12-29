import { Logger } from "../log.js";
import { DepacketizeVideoPipe } from "../video/depackitize_video_pipe.js";
import { VideoMediaStreamTrackGeneratorPipe } from "../video/media_stream_track_generator_pipe.js";
import { VideoMediaStreamTrackProcessorPipe } from "../video/media_stream_track_processor_pipe.js";
import { VideoDecoderPipe } from "../video/video_decoder_pipe.js";
import { VideoTrackGeneratorPipe } from "../video/video_track_generator.js";

export interface Pipe {
    readonly implementationName: string

    getBase(): Pipe | null
}
export interface PipeStatic extends InputPipeStatic {
    readonly type: string

    new(base: any, logger?: Logger): Pipe
}

export interface InputPipeStatic {
    readonly baseType: string
}
export interface OutputPipeStatic {
    readonly type: string

    new(logger?: Logger): Pipe
}

export type Pipeline = {
    pipes: Array<string | PipeStatic>
}

export function pipelineToString(pipeline: Pipeline): string {
    return pipeline.pipes.map(pipe => pipeName(pipe)).join(" -> ")
}

function pipes(): Array<PipeStatic> {
    return [
        // Video
        DepacketizeVideoPipe,
        VideoMediaStreamTrackGeneratorPipe,
        VideoMediaStreamTrackProcessorPipe,
        VideoDecoderPipe,
        VideoTrackGeneratorPipe,
        // TODO: Audio
        // DepacketizeAudioPipe,
    ]
}

export function pipeName(pipe: string | PipeStatic): string {
    if (typeof pipe == "string") {
        return pipe
    } else {
        return pipe.name
    }
}
export function getPipe(pipe: string | PipeStatic): PipeStatic | null {
    if (typeof pipe == "string") {
        const foundPipe = pipes().find(check => check.name == pipe)

        return foundPipe ?? null
    } else {
        return pipe
    }
}

export function buildPipeline(base: OutputPipeStatic, pipeline: Pipeline, logger?: Logger): Pipe | null {
    let previousPipeStatic = base
    let pipe = new base(logger)

    for (let index = pipeline.pipes.length - 1; index >= 0; index--) {
        const currentPipeValue = pipeline.pipes[index]
        const currentPipe = getPipe(currentPipeValue)

        if (!currentPipe) {
            logger?.debug(`Failed to construct pipe because it isn't registered: ${pipeName(currentPipeValue)}`)
            return null
        }

        if (previousPipeStatic && currentPipe.baseType != previousPipeStatic.type) {
            logger?.debug(`Failed to create pipeline "${pipelineToString(pipeline)}" because of baseType of "${currentPipe.name}" is "${currentPipe.baseType}", but it's trying to connect with "${previousPipeStatic.type}"`)
            return null
        }

        previousPipeStatic = currentPipe
        pipe = new currentPipe(pipe, logger)
    }

    return pipe
}