//! Tests unitaires du portage Monkey C.
//!
//! Les valeurs attendues sont celles des tests du cœur Rust
//! (crates/mpacer-core/src/*.rs) : si le portage s'écarte, le test échoue.
//!
//! Exécution (nécessite un profil d'appareil installé) :
//!
//!   pwsh ./garmin/build.ps1 -Test -Device fr965
//!
//! Sortie dans le simulateur Connect IQ.

using Toybox.Test;
using Toybox.Lang;

(:test)
function testFormatDuration(logger) {
    Test.assertEqual("0:00", MpacerUnits.formatDuration(0.0));
    Test.assertEqual("1:02", MpacerUnits.formatDuration(62.4));
    Test.assertEqual("42:15", MpacerUnits.formatDuration(2535.0));
    Test.assertEqual("1:23:45", MpacerUnits.formatDuration(5025.0));
    Test.assertEqual("--:--", MpacerUnits.formatDuration(null));
    return true;
}

(:test)
function testFormatPace(logger) {
    Test.assertEqual("5:41", MpacerUnits.formatPace(341.4));
    Test.assertEqual("1:00", MpacerUnits.formatPace(59.6));
    Test.assertEqual("--:--", MpacerUnits.formatPace(null));
    return true;
}

(:test)
function testPaceSpeedRoundtrip(logger) {
    var speed = MpacerUnits.speedFromPace(341.0, false);
    var pace = MpacerUnits.paceFromSpeed(speed, false);
    Test.assertEqual(341.0, pace);
    // 1 mile = 1609,344 m : 9:00 min/mi ~ 2,98 m/s
    var mileSpeed = MpacerUnits.speedFromPace(540.0, true);
    Test.assertMessage(mileSpeed > 2.97 && mileSpeed < 2.99, "vitesse mile = " + mileSpeed.toString());
    return true;
}

(:test)
function testDistanceFormatting(logger) {
    Test.assertEqual("10.05 km", MpacerUnits.formatDistance(10050.0, false));
    Test.assertEqual("1.00 mi", MpacerUnits.formatDistance(1609.344, true));
    Test.assertEqual("350 m", MpacerUnits.formatDistanceShort(350.0, false));
    return true;
}

(:test)
function testHaversine(logger) {
    // Paris - Londres : ~343,5 km
    var parisLondon = MpacerGeo.haversineM(48.8566, 2.3522, 51.5074, -0.1278);
    Test.assertMessage(parisLondon > 341000.0 && parisLondon < 346000.0, "Paris-Londres = " + parisLondon.toString());
    // Un degré de latitude : ~111,2 km
    var degree = MpacerGeo.haversineM(45.0, 3.0, 46.0, 3.0);
    Test.assertMessage(degree > 110700.0 && degree < 111700.0, "degre = " + degree.toString());
    return true;
}

(:test)
function testGpsMonitorReachesGreen(logger) {
    var monitor = new MpacerGpsMonitor();
    Test.assertEqual(:acquiring, monitor.status);
    monitor.push(0, 45.0, 3.0, 30.0);
    Test.assertEqual(:poor, monitor.status);
    monitor.push(1000, 45.00001, 3.0, 5.0);
    monitor.push(2000, 45.00002, 3.0, 5.0);
    Test.assertEqual(:poor, monitor.status);
    monitor.push(3000, 45.00003, 3.0, 5.0);
    Test.assertEqual(:good, monitor.status);
    Test.assertEqual(:green, monitor.light());
    return true;
}

(:test)
function testGpsMonitorRejectsJump(logger) {
    var monitor = new MpacerGpsMonitor();
    monitor.push(0, 45.0, 3.0, 5.0);
    var result = monitor.push(1000, 45.05, 3.0, 5.0);   // ~5,5 km en 1 s
    Test.assertEqual(false, result[1]);
    Test.assertEqual(1, monitor.rejectedSamples);
    return true;
}

(:test)
function testSteadyPaceIsThreeMinutesPerKm(logger) {
    var engine = new MpacerPaceEngine();
    var distance = 0.0;
    for (var second = 0; second < 180; second++) {
        distance = distance + 5.5556;
        engine.push(second * 1000, distance);
    }
    var pace = engine.currentPace(false);
    Test.assertMessage(pace > 179.0 && pace < 181.0, "allure = " + pace.toString());
    return true;
}

(:test)
function testLapIsEmittedAtEachKilometer(logger) {
    var tracker = new MpacerLapTracker();
    tracker.start(0.0);
    for (var step = 1; step <= 100; step++) {
        tracker.update(step * 3.0, step * 10.0);
    }
    Test.assertEqual(1, tracker.lapCount());
    var pace = tracker.previousLapPaceSPerKm();
    Test.assertMessage(pace > 299.9 && pace < 300.1, "allure du tour = " + pace.toString());
    return true;
}

(:test)
function testShadowRunnerPlanIsExactAtTheFinish(logger) {
    var plan = new MpacerRacePlan(42195.0, 14400.0);
    plan.setNegativeSplit(true, 0.03);
    var start = plan.paceAtDistanceSPerKm(0.0);
    var finish = plan.paceAtDistanceSPerKm(42195.0);
    Test.assertMessage(start > 350.0 && start < 352.0, "depart = " + start.toString());
    Test.assertMessage(finish > 329.0 && finish < 331.0, "arrivee = " + finish.toString());
    var total = plan.timeAtDistanceS(42195.0);
    Test.assertMessage(total > 14399.9 && total < 14400.1, "temps total = " + total.toString());
    return true;
}

(:test)
function testAssistantPredictsFinishTime(logger) {
    var assistant = new MpacerAssistant();
    assistant.configure(:predict_finish, 10000.0, null, false, 0.0);
    var panel = assistant.update(600.0, 2000.0, 300.0, false);
    Test.assertEqual(true, panel[:visible]);
    // 8 km restants a 5:00/km plus 10 min ecoulees = 50 min
    Test.assertMessage(panel[:estimated_finish_s] > 2999.0 && panel[:estimated_finish_s] < 3001.0, "finish = " + panel[:estimated_finish_s].toString());
    return true;
}

(:test)
function testBestEffortOfASteadyRun(logger) {
    var track = new MpacerTrack(4000);
    var distance = 0.0;
    for (var second = 0; second <= 3600; second++) {
        if (second % 10 == 0) {
            track.push(second * 1000, distance, 45.0, 3.0, null);
        }
        distance = distance + 3.0;
    }
    var efforts = track.bestEfforts([[1000.0, "1 km"]]);
    Test.assertEqual(1, efforts.size());
    var time = efforts[0][:time_s];
    // 1000 m a 3 m/s = 333,3 s
    Test.assertMessage(time > 332.0 && time < 335.0, "meilleur km = " + time.toString());
    return true;
}

(:test)
function testCardioZones(logger) {
    var cardio = new MpacerCardio(190);
    Test.assertEqual(0, cardio.zoneOf(90));
    Test.assertEqual(1, cardio.zoneOf(100));
    Test.assertEqual(3, cardio.zoneOf(140));
    Test.assertEqual(5, cardio.zoneOf(180));
    return true;
}
