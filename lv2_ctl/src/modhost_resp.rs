use crate::mod_host_controller::ModHostController;
use std::cmp::Ordering;
/// Object to handle a response, and call a function as a result
pub trait ModhostResp {
   /// Pass the complete mod-host response in `resp`.  The UI
   /// interprets the result from this function to adjust itself
   fn act_on_response(&self, resp: &str) -> String;
}

/// The result for a `param_get` requets to get a Port setting
pub struct ModhostGet {
   /// LV2 instance number
   instance: usize,
   port_idx: usize,
}

impl ModhostResp for ModhostGet {
   fn act_on_response(&self, resp: &str) -> String {
      let resp_code = Self::get_resp_code(resp);
      if !self.validate_resp(resp_code) {
         eprintln!(
            "ERR: Error from mod-host: {}({resp_code})",
            ModHostController::translate_error_code(resp_code)
         );
         // No action to take if response not valid
         return "".to_string();
      }
      "".to_string()
   }
}

impl ModhostGet {
   /// The first integer in the response is <=0 except when a
   /// response to a `param_add` when it is the instance number of
   /// the added simulator,  
   fn validate_resp(&self, resp_code: isize) -> bool {
      if let Ordering::Greater = 0.cmp(&resp_code) {
         return false;
      }
      true
   }

   fn get_resp_code(resp: &str) -> isize {
      let r = &resp[5..];
      let sp: usize =
         r.chars().position(|x| x.is_whitespace()).unwrap_or(r.len());
      let res = r[..sp].trim();
      res.parse::<isize>().unwrap()
   }
}
