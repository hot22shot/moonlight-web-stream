import { PutHostRequest } from "../../api_bindings.js"
import { FormModal } from "../modal.js"

export class AddHostModal extends FormModal<PutHostRequest> {

    private addressElement: HTMLInputElement = document.createElement("input")
    private httpPortElement: HTMLInputElement = document.createElement("input")

    constructor() {
        super()

        this.addressElement.type = "text"

        this.httpPortElement.type = "text"
        this.httpPortElement.inputMode = "numeric"
    }

    reset(): void {
        this.addressElement.value = ""
        this.httpPortElement.value = ""
    }
    submit(): PutHostRequest | null {
        const address = this.addressElement.value
        const httpPort = this.httpPortElement.valueAsNumber

        return {
            address,
            http_port: httpPort
        }
    }

    mountForm(form: HTMLFormElement): void {
        form.appendChild(this.addressElement)
        form.appendChild(this.httpPortElement)
    }
}