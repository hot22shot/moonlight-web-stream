import { PutHostRequest } from "../../api_bindings.js"
import { FormModal } from "../modal/form.js"

export class AddHostModal extends FormModal<PutHostRequest> {

    private addressLabel: HTMLLabelElement = document.createElement("label")
    private addressInput: HTMLInputElement = document.createElement("input")

    private httpPortLabel: HTMLLabelElement = document.createElement("label")
    private httpPortInput: HTMLInputElement = document.createElement("input")

    constructor() {
        super()

        this.addressLabel.innerText = "Address"
        this.addressInput.type = "text"
        this.addressLabel.appendChild(this.addressInput)

        this.httpPortLabel.innerText = "Port"
        this.httpPortInput.type = "text"
        this.httpPortInput.inputMode = "numeric"
        this.httpPortLabel.appendChild(this.httpPortInput)
    }

    reset(): void {
        this.addressInput.value = ""
        this.httpPortInput.value = ""
    }
    submit(): PutHostRequest | null {
        const address = this.addressInput.value
        const httpPort = this.httpPortInput.valueAsNumber

        return {
            address,
            http_port: httpPort
        }
    }

    mountForm(form: HTMLFormElement): void {
        form.appendChild(this.addressLabel)
        form.appendChild(this.httpPortLabel)
    }
}