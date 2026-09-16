declare namespace Login{

    interface LoginReqest{
        mail:string
        password:string
        vcode?:string
    }

	interface LoginResponse extends User.Info{
		token :string
	}

    interface ResetPasswordByVCodeReqest extends System.Register.SendRegisterVcodeRquest{
    }

}
