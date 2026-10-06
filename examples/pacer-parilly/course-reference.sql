-- Course de reference M-pacer : pacer 1 h a 4:00/km, 3 tours de la boucle du parc de Parilly.
-- Rejouable sans risque : la course n'est creee que si elle n'existe pas deja
-- (meme utilisateur, meme nom). Execution sur la base M-pacer :
--   kubectl exec -i -n mpacer mpacer-pg-1 -- psql -U postgres -d mpacer -f -
\set ON_ERROR_STOP on

BEGIN;

WITH nouvelle AS (
    INSERT INTO races (
        id, user_id, name, start_at_ms, distance_m, discipline, location, start_location,
        bib_number, bib_pickup_at_ms, bib_pickup_location, live_url, registration_url,
        website_url, latitude, longitude, hotel_name, hotel_address, hotel_phone, hotel_url,
        hotel_booked, hotel_check_in_ms, hotel_check_out_ms, lodging_notes, nutrition_notes,
        important_info, notes, goal_time_s, created_at_ms, updated_at_ms
    )
    SELECT
        gen_random_uuid()::text,
        u.id,
        $m$Pacer de référence — 1 h à 4:00/km (parc de Parilly)$m$,
        NULL,
        15000,
        $m$Route$m$,
        $m$Parc de Parilly — Saint-Priest / Bron / Vénissieux$m$,
        $m$Parc de Parilly (boucle du 5 & 10 km)$m$,
        NULL, NULL, NULL, NULL, NULL,
        $m$https://5et10kmdeparilly.odoo.com/parcours$m$,
        45.717270,
        4.900305,
        NULL, NULL, NULL, NULL,
        FALSE, NULL, NULL, NULL, NULL,
        $m$Pacer de référence : 1 h à 4:00 min/km = 15,00 km = 3 tours de la boucle de 5 km du parc de Parilly (4 999 m).
Réglages montre : mode Shadow runner, distance 15 000 m, temps cible 3 600 s, negative split 0 % (ou 3 % : de 4:07 à 3:53).
Passages : 5 km en 20:00, 10 km en 40:00, arrivée 15,00 km en 1:00:00.
Boucle : tracé officiel du 5 & 10 km de Parilly (187 points, 4 999 m), décrit comme roulant.$m$,
        $m$Exemple de pacer préparé pour tester le shadow runner de la montre.
Traces : examples/pacer-parilly/pacer-1h-4min-km-15km.gpx (3 tours horodatés à 4:00/km) et parilly-boucle-5km.gpx.
Référence : cargo run -q -p mpacer-sim -- --mode plan --distance 15000 --time 3600 --split 0 --minutes 70 --every 600
Attendu : 15,00 km, allure moyenne 4:00, 15 tours de 1 km, 5 km en 19:58, 10 km en 39:59.$m$,
        3600,
        (extract(epoch from now()) * 1000)::bigint,
        (extract(epoch from now()) * 1000)::bigint
    FROM users u
    WHERE u.email = 'joseph@zacharie.org'
      AND NOT EXISTS (
          SELECT 1 FROM races r
          WHERE r.user_id = u.id AND r.name = $m$Pacer de référence — 1 h à 4:00/km (parc de Parilly)$m$
      )
    RETURNING id, user_id
)
INSERT INTO race_tasks (id, race_id, user_id, label, due_at_ms, done, done_at_ms, position, created_at_ms)
SELECT gen_random_uuid()::text, n.id, n.user_id, t.label, NULL, FALSE, NULL, t.position,
       (extract(epoch from now()) * 1000)::bigint
FROM nouvelle n
CROSS JOIN (VALUES
    (0, $m$Régler le shadow runner sur la montre (15 km / 1 h 00)$m$),
    (1, $m$Vérifier le plan en simulation (mpacer-sim)$m$),
    (2, $m$Charger le GPX de la boucle de Parilly$m$),
    (3, $m$Repérer la boucle de 5 km et ses repères$m$),
    (4, $m$Faire la séance de référence (3 tours à 4:00/km)$m$)
) AS t(position, label);

COMMIT;

SELECT r.id, r.name, r.distance_m, r.goal_time_s, r.discipline, r.location, r.latitude, r.longitude, r.website_url, r.start_at_ms
FROM races r JOIN users u ON u.id = r.user_id
WHERE u.email = 'joseph@zacharie.org';

SELECT t.position, t.label, t.done
FROM race_tasks t JOIN races r ON r.id = t.race_id JOIN users u ON u.id = r.user_id
WHERE u.email = 'joseph@zacharie.org'
ORDER BY t.position;
