import json

def get_address_from_hotkeys(identifier: str) -> str:# identifier là tên wallet truyền vào hoặc là giá trị địa chỉ trực tiếp
    file_path = "moderntensor/kickoff/hotkeys.json"
    
    try:
        with open(file_path, "r", encoding="utf-8") as file:
            data = json.load(file)
            
            if "hotkeys" in data:
                for wallet_name, wallet_info in data["hotkeys"].items():
                    # data["hotkeys"].items() là một dict gồm key là wallet name và value là 1 đối tượng gồm 
                    #   "address": "addr_test1qz9twyn8njyux586y7c92c3ldwk33xgutw4qjtjahjnqqytyau03kld4qfhqnd77r8jcmr39zn3cpr003pxccr5sjsnq9m4n4c",
                    #   "encrypted_data": "gAAAAABn1V2c_Huoz8Zava1h5f0ZTM5jtGIFJnsaI9H0bQ0zFTfJDY8XD93Xs9W3VqSqDCSPCUEbshsx6k1EJUoOMX1afCUJGgh0PIJqDaucuVRlXdNkgBBl-QUqDdgm8ztNPhdWE4EGdnh93XUPU33CKFiyS2NSEzjVXy6P10RlQ8wHyfKxzFOvIbI4zHh2j1TKq8iZDZiV_TxqMWEbwFI__SSBJiGBy9POw3sI2MRt9vPRtNIILb8pK4FhOHqHwQ-Jd5Vu1we3Fj_7gWVVSFBN3MNMuPexwwAmzWlfhmPhKMlZ9x0xYLdnmvpBYhKBe9RD383qruk3oWVo6dccaifpYA0xE61AL_OGCx3EbyCkvqnSo8-o6VSpZ3uqV3lc95Rlq6TVbHlFRElrqlQcVwALNYzVHIdu2ZT0No5J-VGTXLlfWt2p7g44UcIpqtXCw_fdT46kJJZeiKT9F1Ll-0thjcs-eWSiZxqtEhseiH2XbK_MyAL1d_5I8H96JKLWAtA9LNbyNR6y9zKXvWsG7LBFhrlncJW-ioR911Cdx6GyUSc3xUQJUxYypSKxT7gLBs_VfxlJHKbOOcU_DuYf4fxIPJqUCMaSJLx3dyYRVZQy4nljg3DPILORse0Yj-ne_6Ug-LpdwZlHcG-_dJTOWemtzJfXbTKo5Skd-0g4Sve5kA-mnBN8YKstGDEtVwK79IQzfmBQZ6_cb9e7j956fmuxk8ZWEVobFy2e24_kW1Ncz-YH2w1UZH3D31HKkk5yM_r5hzUWl_wf6kLITccDqXOy2Wjaf1qKB7dBCQ1qjbmjN8J2kCv60Y_vvy8pcPZG6uNumBtXBAFPy9y-E2ztyjw4_uPsCHsIxpOgrZESJMXeZ4OeRaby-66yl0W-rbn47iFR2ShI2TSnqUaPsvTC96LiXI0m4yo6wod8UYD6VqLBO-mIwhKoHx77pKYqLYP5mCygmXRscRrpEqS9PQ5BryqgRb9HMaz6d7wXlaiz59MiYZqeGZQ6wtGh2cjaOMyF8FroTLhSETYG"
                    # }
                    
                    if wallet_name == identifier:
                        return wallet_info.get("address", "Address not found")
                    elif wallet_info.get("address") == identifier:
                        return identifier
                return "Wallet not found"
            else:
                return "Invalid data structure"
    except FileNotFoundError:
        return "File not found"
    except json.JSONDecodeError:
        return "Invalid JSON format"

# Ví dụ sử dụng:
wallet_identifier = "hk1"  # hoặc truyền vào địa chỉ ví
wallet_identifier = "addr_test1qz9twyn8njyux586y7c92c3ldwk33xgutw4qjtjahjnqqytyau03kld4qfhqnd77r8jcmr39zn3cpr003pxccr5sjsnq9m4n4c"  # hoặc truyền vào địa chỉ ví
print(get_address_from_hotkeys(wallet_identifier))
