
export interface ByteSerializable { }

export class ByteBuffer {
    private position: number = 0
    private limit: number = 0
    private buffer: Uint8Array

    constructor(length?: number);
    constructor(buffer: Uint8Array);
    constructor(value?: number | Uint8Array) {
        if (value instanceof Uint8Array) {
            this.buffer = value
        } else {
            this.buffer = new Uint8Array(value ?? 0)
        }
    }

    private bytesUsed(amount: number, reading: boolean) {
        this.position += amount
        if (reading && this.position > this.limit) {
            throw "failed to read over the limit"
        }
    }

    putU8Array(data: Array<number>) {
        this.buffer.set(data, this.position)
        this.bytesUsed(data.length, false)
    }

    putU8(data: number) {
        this.putU8Array([data])
    }
    putBool(data: boolean) {
        this.putU8(data ? 1 : 0)
    }

    putUtf8(text: string) {
        const encoder = new TextEncoder()
        const result = encoder.encodeInto(text, this.buffer)

        this.bytesUsed(result.written, false)
        if (result.read != text.length) {
            throw "failed to put utf8 text"
        }
    }

    get(buffer: Uint8Array, offset: number, length: number) {
        buffer.set(this.buffer.slice(this.position, this.position + length), offset)
        this.bytesUsed(length, true)
    }

    getU8(): number {
        const byte = this.buffer[this.position]
        this.bytesUsed(1, true)
        return byte
    }

    reset() {
        this.position = 0
        this.limit = 0
    }
    flip() {
        this.limit = this.position
        this.position = 0
    }
    getPosition() {
        return this.position
    }

    getReadBuffer(): Uint8Array {
        return this.buffer.slice(0, this.limit)
    }
}