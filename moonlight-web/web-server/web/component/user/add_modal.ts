import { PostUserRequest, UserRole } from "../../api_bindings.js";
import { InputComponent, SelectComponent } from "../input.js";
import { FormModal } from "../modal/form.js";
import { createSelectRoleInput } from "./role_select.js";

export class AddUserModal extends FormModal<PostUserRequest> {

    private header: HTMLElement = document.createElement("h2")

    private name: InputComponent
    private defaultPassword: InputComponent
    private role: SelectComponent

    constructor() {
        super()

        this.header.innerText = "User"

        // TODO: prevent empty name or password
        this.name = new InputComponent("userName", "text", "Name")

        this.defaultPassword = new InputComponent("userPassword", "text", "Default Password")

        this.role = createSelectRoleInput("User")
    }

    mountForm(form: HTMLFormElement): void {
        form.appendChild(this.header)
        this.name.mount(form)
        this.defaultPassword.mount(form)
        this.role.mount(form)
    }

    reset(): void {
        this.name.reset()
        this.defaultPassword.reset()
        this.role.reset()
    }
    submit(): PostUserRequest | null {
        const name = this.name.getValue()
        const password = this.defaultPassword.getValue()
        const role = this.role.getValue() as UserRole

        return {
            name,
            password,
            role,
        }
    }
}