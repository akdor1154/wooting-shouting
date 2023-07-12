
use sqlite;

use crate::hid::AnalogueReading;

pub fn sqlite_connection() -> Result<sqlite::Connection, anyhow::Error> {
    let c = sqlite::open("recordings.sqlite")?;

    c.execute("--sql
        create table if not exists events (
            session_epoch integer,
            ts real,
            ts_secs_rel real,
            char text,
            value real
        )
        strict
    ")?;

    return Ok(c);
}

pub struct Recorder<'a> {
    events_epoch: Option<std::time::Instant>,
    unix_epoch: std::time::Instant,
    stmt: sqlite::Statement<'a>
}
impl <'a> Recorder<'a> {
    pub fn new(c: &'a sqlite::Connection) -> Self {

        let stmt = c.prepare("--sql
            insert into events
                (session_epoch, ts, ts_secs_rel, char, value)
            values
                (?, ?, ?, ?, ?)
        ").unwrap();


        let unix_epoch = {
            let (now_sys, now_inst) = (std::time::SystemTime::now(), std::time::Instant::now());
            let since_epoch = now_sys.duration_since(std::time::UNIX_EPOCH).unwrap();
            now_inst - since_epoch
        };


        Recorder{unix_epoch, events_epoch: None, stmt: stmt}

    }

    pub fn record(&mut self, a: &AnalogueReading) {
        let events_epoch = match self.events_epoch {
            Some(e) => e,
            None => {
                self.events_epoch = Some(a.ts);
                a.ts
            }
        };

        let diff = a.ts - events_epoch;

        let key = format!("{:?}", input_linux::Key::from_code(a.scancode).unwrap());

        self.stmt.bind::<&[(_, sqlite::Value)]>(&[
            (1, i64::try_from(events_epoch.duration_since(self.unix_epoch).as_secs()).unwrap().into()),
            (2, a.ts.duration_since(self.unix_epoch).as_secs_f64().into()),
            (3, diff.as_secs_f64().into()),
            (4, key.into()),
            (5, f64::from(a.value).into())
        ]).unwrap();
        self.stmt.next().unwrap();
        self.stmt.reset().unwrap();
    }
}
