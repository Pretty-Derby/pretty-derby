mod routine;
use log::{debug, info};
use routine::*;

use chrono::{Duration, Local};
use rand::{thread_rng, Rng};
use reqwest::{header::*, Client};
use serde::Deserialize;
use serde_json::json;
use sha1::{digest::FixedOutputReset, Digest, Sha1};
use std::{collections::HashMap, error::Error};

const URL_CURRENT: &'static str = "https://cpes.legym.cn/education/semester/getCurrent";
const URL_GETRUNNINGLIMIT: &'static str = "https://cpes.legym.cn/running/app/getRunningLimit";
const URL_GETVERSION: &'static str =
    "https://cpes.legym.cn/authorization/mobileApp/getLastVersion?platform=2";
const URL_LOGIN: &'static str = "https://cpes.legym.cn/authorization/user/manage/login";
const URL_UPLOADRUNNING: &'static str = "https://cpes.legym.cn/running/app/v2/uploadRunningDetails";

const ORGANIZATION: HeaderName = HeaderName::from_static("organization");
const HEADERS: [(HeaderName, &'static str); 9] = [
    (ACCEPT, "*/*"),
    (ACCEPT_ENCODING, "gzip, deflate, br"),
    (ACCEPT_LANGUAGE, "zh-CN, zh-Hans;q=0.9"),
    (AUTHORIZATION, ""),
    (CONNECTION, "keep-alive"),
    (CONTENT_TYPE, "application/json"),
    (HOST, "cpes.legym.cn"),
    (ORGANIZATION, ""),
    (USER_AGENT, "Mozilla/5.0 (iPhone; CPU iPhone OS 15_4_1 like Mac OSX) AppleWebKit/605.1.15 (KHTML, like Gecko) Mobile/15E148 Html15Plus/1.0 (Immersed/47) uni-app"),
];

const CALORIE_PER_MILEAGE: f64 = 58.3;
const SALT: &'static str = "itauVfnexHiRigZ6";

pub struct Account {
    client: Client,
    daily: f64,
    day: f64,
    end: f64,
    hasher: Sha1,
    headers: HeaderMap,
    id: String,
    limitation: String,
    organization: String,
    scoring: i64,
    semester: String,
    start: f64,
    token: String,
    version: String,
    week: f64,
    weekly: f64,
}

impl Account {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let mut headers = HeaderMap::new();
        for (key, val) in HEADERS {
            headers.insert(key, val.parse().unwrap());
        }
        Ok(Self {
            client: Client::new(),
            daily: 0.,
            day: 0.,
            end: 0.,
            hasher: Sha1::new(),
            headers,
            id: String::new(),
            limitation: String::new(),
            organization: String::new(),
            scoring: 0,
            semester: String::new(),
            start: 0.,
            token: String::new(),
            version: String::new(),
            week: 0.,
            weekly: 0.,
        })
    }

    pub async fn login(
        &mut self,
        username: String,
        password: String,
    ) -> Result<(), Box<dyn Error>> {
        self.get_token(username, password).await?;
        self.get_current().await?;
        self.get_version().await?;
        self.get_running_limit().await?;
        Ok(())
    }

    async fn get_token(
        &mut self,
        username: String,
        password: String,
    ) -> Result<(), Box<dyn Error>> {
        let signdigital = {
            self.hasher
                .update((username.to_string() + &password + "1" + SALT).as_bytes());
            hex::encode(self.hasher.finalize_fixed_reset().to_vec())
        };
        let json = json!({
            "entrance": "1",
            "password": &password.to_string(),
            "signDigital": &signdigital.to_string(),
            "userName": &username.to_string(),
        });

        debug!("Login json: {:#?}", json);

        let res = self
            .client
            .post(URL_LOGIN)
            .headers(self.headers.clone())
            .json(&json)
            .send()
            .await?
            .error_for_status()?;

        #[derive(Deserialize, Debug)]
        #[allow(non_snake_case)]
        struct LoginData {
            id: String,
            accessToken: String,
            campusId: String,
        }

        #[derive(Deserialize)]
        struct LoginResult {
            data: LoginData,
        }

        debug!("Login response: {:#?}", res);
        let data = res
            .json::<LoginResult>()
            .await
            .or(Err("Login failed"))?
            .data;

        self.id = data.id;
        self.token = data.accessToken;
        self.organization = data.campusId;
        *self.headers.get_mut(ORGANIZATION).unwrap() = self.organization.parse().unwrap();
        *self.headers.get_mut(AUTHORIZATION).unwrap() =
            ("Bearer ".to_owned() + &self.token).parse().unwrap();

        info!("Get token successful!");
        Ok(())
    }

    async fn get_current(&mut self) -> Result<(), Box<dyn Error>> {
        let res = self
            .client
            .get(URL_CURRENT)
            .headers(self.headers.clone())
            .send()
            .await?
            .error_for_status()?;

        #[derive(Deserialize, Debug)]
        #[allow(non_snake_case)]
        struct CurrentData {
            id: String,
        }

        #[derive(Deserialize)]
        struct CurrentResult {
            data: CurrentData,
        }

        debug!("Current response: {:#?}", res);
        let data = res.json::<CurrentResult>().await?.data;

        self.semester = data.id;

        info!("Get current successful!");
        Ok(())
    }

    async fn get_version(&mut self) -> Result<(), Box<dyn Error>> {
        // Get Version
        let res = self
            .client
            .get(URL_GETVERSION)
            .headers(self.headers.clone())
            .send()
            .await?
            .error_for_status()?;

        debug!("Version response: {:#?}", res);
        #[derive(Deserialize, Debug)]
        #[allow(non_snake_case)]
        struct VersionData {
            versionLabel: String,
        }

        #[derive(Deserialize)]
        struct VersionResult {
            data: VersionData,
        }
        let data = res.json::<VersionResult>().await?.data;

        self.version = data.versionLabel;

        info!("Get version successful!");
        Ok(())
    }

    async fn get_running_limit(&mut self) -> Result<(), Box<dyn Error>> {
        let json = json!({
            "semesterId": self.semester,
        });
        debug!("Running limits json: {:#?}", json);

        let res = self
            .client
            .post(URL_GETRUNNINGLIMIT)
            .headers(self.headers.clone())
            .json(&json)
            .send()
            .await?
            .error_for_status()?;

        #[derive(Deserialize, Debug)]
        #[allow(non_snake_case)]
        struct RunningLimitsData {
            dailyMileage: f64,
            effectiveMileageEnd: f64,
            effectiveMileageStart: f64,
            limitationsGoalsSexInfoId: String,
            scoringType: i64,
            totalDayMileage: String,
            totalWeekMileage: String,
            weeklyMileage: f64,
        }

        #[derive(Deserialize)]
        struct RunningLimitsResult {
            data: RunningLimitsData,
        }

        debug!("Running limits response: {:#?}", res);
        let data = res.json::<RunningLimitsResult>().await?.data;

        self.daily = data.dailyMileage;
        self.day = data.totalDayMileage.parse()?;
        self.end = data.effectiveMileageEnd;
        self.limitation = data.limitationsGoalsSexInfoId;
        self.scoring = data.scoringType;
        self.start = data.effectiveMileageStart;
        self.week = data.totalWeekMileage.parse()?;
        self.weekly = data.weeklyMileage;

        info!("Get running limitation successful!");
        Ok(())
    }

    pub async fn upload_running(
        &mut self,
        mileage: f64,
        routefile: Option<String>,
    ) -> Result<(), Box<dyn Error>> {
        let headers: HeaderMap<HeaderValue> = (&HashMap::from([
            (
                ACCEPT_ENCODING,
                "br;q=1.0, gzip;q=0.9, deflate;q=0.8".parse().unwrap(),
            ),
            (
                ACCEPT_LANGUAGE,
                "zh-Hans-HK;q=1.0, zh-Hant-HK;q=0.9, yue-Hant-HK;q=0.8"
                    .parse::<HeaderValue>()
                    .unwrap(),
            ),
            (
                AUTHORIZATION,
                ("Bearer ".to_owned() + &self.token).parse().unwrap(),
            ),
            (
                USER_AGENT,
                "QJGX/3.8.2 (com.ledreamer.legym; build:30000812; iOS 16.0.2) Alamofire/5.6.2"
                    .parse()
                    .unwrap(),
            ),
            (ACCEPT, "*/*".parse().unwrap()),
            (CONNECTION, "keep-alive".parse().unwrap()),
            (CONTENT_TYPE, "application/json".parse().unwrap()),
            (HOST, "cpes.legym.cn".parse().unwrap()),
        ]))
            .try_into()?;

        let mut mileage = mileage
            .min(self.daily - self.day)
            .min(self.weekly - self.week)
            .min(self.end);

        if mileage < self.start {
            return Err(String::from("Effective mileage too low").into());
        }

        let keeptime;
        let pace;
        {
            // WARN: Must make sure that the rng dies before the await call
            let mut rng = thread_rng();
            mileage += rng.gen_range(-0.02..-0.001);
            keeptime = (mileage * 1000.0 / 3.0) as i64 + rng.gen_range(-15..15);
            pace = 0.6 + rng.gen_range(-0.05..0.05);
        }
        let end_time = Local::now();
        let start_time = end_time - Duration::seconds(keeptime);

        let signdigital = {
            self.hasher.update(
                (mileage.to_string()
                    + "1"
                    + &start_time.format("%Y-%m-%d %H:%M:%S").to_string()
                    + &((CALORIE_PER_MILEAGE * mileage) as i64).to_string()
                    + &((keeptime as f64 / mileage) as i64 * 1000).to_string()
                    + &keeptime.to_string()
                    + &((mileage * 1000. / pace / 2.) as i64).to_string()
                    + &mileage.to_string()
                    + "1"
                    + &SALT.to_string())
                    .as_bytes(),
            );
            hex::encode(self.hasher.finalize_fixed_reset().to_vec())
        };
        let json = json!({
            "appVersion": self.version,
            "avePace": (keeptime as f64 / mileage) as i64 * 1000,
            "calorie": (CALORIE_PER_MILEAGE * mileage) as i64,
            "deviceType": "iPhone 13 Pro",
            "effectiveMileage": mileage,
            "effectivePart": 1,
            "endTime": end_time.format("%Y-%m-%d %H:%M:%S").to_string(),
            "gpsMileage": mileage,
            "keepTime": keeptime,
            "limitationsGoalsSexInfoId": self.limitation,
            "paceNumber": (mileage * 1000. / pace / 2.) as i64,
            "paceRange": pace,
            "routineLine": get_routine(mileage, routefile)?,
            "scoringType": self.scoring,
            "semesterId": self.semester,
            "signDigital": signdigital,
            "signPoint": [],
            "startTime": start_time.format("%Y-%m-%d %H:%M:%S").to_string(),
            "systemVersion": "16.0.2",
            "totalMileage": mileage,
            "totalPart": 1,
            "type": "范围跑",
            "uneffectiveReason": "",
        });

        debug!("Upload running json: {}", json.to_string());

        self.client
            .post(URL_UPLOADRUNNING)
            .headers(headers)
            .json(&json)
            .send()
            .await?
            .error_for_status()?;

        info!("Upload running successful!");
        Ok(())
    }
}
