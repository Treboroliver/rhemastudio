use crate::book::Book;
use crate::verse_ref::VerseRef;

pub const TOTAL_CANONICAL_BOOKS: usize = 66;
pub const TOTAL_CANONICAL_CHAPTERS: usize = 1189;
pub const TOTAL_CANONICAL_VERSES: usize = 31102;
pub const OLD_TESTAMENT_VERSES: usize = 23145;
pub const NEW_TESTAMENT_VERSES: usize = 7957;

pub struct BookMeta {
    pub osis: &'static str,
    pub name_pt: &'static str,
    pub name_en: &'static str,
    pub abbr_pt: &'static [&'static str],
    pub abbr_en: &'static [&'static str],
}

pub const BOOK_METADATA: [BookMeta; 66] = [
    // 1. Gênesis
    BookMeta {
        osis: "Gen",
        name_pt: "Gênesis",
        name_en: "Genesis",
        abbr_pt: &["Gn", "Gen", "Gên"],
        abbr_en: &["Gen", "Ge", "Gn"],
    },
    // 2. Êxodo
    BookMeta {
        osis: "Exod",
        name_pt: "Êxodo",
        name_en: "Exodus",
        abbr_pt: &["Ex", "Êx", "Exo", "Êxo"],
        abbr_en: &["Exo", "Ex", "Exod"],
    },
    // 3. Levítico
    BookMeta {
        osis: "Lev",
        name_pt: "Levítico",
        name_en: "Leviticus",
        abbr_pt: &["Lv", "Lev"],
        abbr_en: &["Lev", "Le", "Lv"],
    },
    // 4. Números
    BookMeta {
        osis: "Num",
        name_pt: "Números",
        name_en: "Numbers",
        abbr_pt: &["Nm", "Num", "Núm"],
        abbr_en: &["Num", "Nu", "Nm", "Nb"],
    },
    // 5. Deuteronômio
    BookMeta {
        osis: "Deut",
        name_pt: "Deuteronômio",
        name_en: "Deuteronomy",
        abbr_pt: &["Dt", "Deut"],
        abbr_en: &["Deut", "De", "Dt"],
    },
    // 6. Josué
    BookMeta {
        osis: "Josh",
        name_pt: "Josué",
        name_en: "Joshua",
        abbr_pt: &["Js", "Jos"],
        abbr_en: &["Josh", "Jos", "Jsh"],
    },
    // 7. Juízes
    BookMeta {
        osis: "Judg",
        name_pt: "Juízes",
        name_en: "Judges",
        abbr_pt: &["Jz", "Jz"],
        abbr_en: &["Judg", "Jdg", "Jg", "Jdgs"],
    },
    // 8. Rute
    BookMeta {
        osis: "Ruth",
        name_pt: "Rute",
        name_en: "Ruth",
        abbr_pt: &["Rt", "Rut"],
        abbr_en: &["Rth", "Ru", "Ruth"],
    },
    // 9. 1 Samuel
    BookMeta {
        osis: "1Sam",
        name_pt: "1 Samuel",
        name_en: "1 Samuel",
        abbr_pt: &["1Sm", "1 Sm", "1Sam", "1 Sam"],
        abbr_en: &["1Sam", "1 Sam", "1Sa", "1 Sa", "1S"],
    },
    // 10. 2 Samuel
    BookMeta {
        osis: "2Sam",
        name_pt: "2 Samuel",
        name_en: "2 Samuel",
        abbr_pt: &["2Sm", "2 Sm", "2Sam", "2 Sam"],
        abbr_en: &["2Sam", "2 Sam", "2Sa", "2 Sa", "2S"],
    },
    // 11. 1 Reis
    BookMeta {
        osis: "1Kgs",
        name_pt: "1 Reis",
        name_en: "1 Kings",
        abbr_pt: &["1Rs", "1 Rs", "1Re", "1 Re"],
        abbr_en: &["1Kgs", "1 Kgs", "1Ki", "1 Ki", "1K"],
    },
    // 12. 2 Reis
    BookMeta {
        osis: "2Kgs",
        name_pt: "2 Reis",
        name_en: "2 Kings",
        abbr_pt: &["2Rs", "2 Rs", "2Re", "2 Re"],
        abbr_en: &["2Kgs", "2 Kgs", "2Ki", "2 Ki", "2K"],
    },
    // 13. 1 Crônicas
    BookMeta {
        osis: "1Chr",
        name_pt: "1 Crônicas",
        name_en: "1 Chronicles",
        abbr_pt: &["1Cr", "1 Cr", "1Cron", "1 Cron"],
        abbr_en: &["1Chr", "1 Chr", "1Ch", "1 Ch"],
    },
    // 14. 2 Crônicas
    BookMeta {
        osis: "2Chr",
        name_pt: "2 Crônicas",
        name_en: "2 Chronicles",
        abbr_pt: &["2Cr", "2 Cr", "2Cron", "2 Cron"],
        abbr_en: &["2Chr", "2 Chr", "2Ch", "2 Ch"],
    },
    // 15. Esdras
    BookMeta {
        osis: "Ezra",
        name_pt: "Esdras",
        name_en: "Ezra",
        abbr_pt: &["Ed", "Esd"],
        abbr_en: &["Ezr", "Ezra"],
    },
    // 16. Neemias
    BookMeta {
        osis: "Neh",
        name_pt: "Neemias",
        name_en: "Nehemiah",
        abbr_pt: &["Ne", "Neh", "Neem"],
        abbr_en: &["Neh", "Ne"],
    },
    // 17. Ester
    BookMeta {
        osis: "Esth",
        name_pt: "Ester",
        name_en: "Esther",
        abbr_pt: &["Et", "Est"],
        abbr_en: &["Esth", "Es", "Est"],
    },
    // 18. Jó
    BookMeta {
        osis: "Job",
        name_pt: "Jó",
        name_en: "Job",
        abbr_pt: &["Jó", "Jb"],
        abbr_en: &["Job", "Jb"],
    },
    // 19. Salmos
    BookMeta {
        osis: "Ps",
        name_pt: "Salmos",
        name_en: "Psalms",
        abbr_pt: &["Sl", "Sal", "Salmo"],
        abbr_en: &["Ps", "Psa", "Psalms", "Psalm"],
    },
    // 20. Provérbios
    BookMeta {
        osis: "Prov",
        name_pt: "Provérbios",
        name_en: "Proverbs",
        abbr_pt: &["Pv", "Prov"],
        abbr_en: &["Prov", "Pro", "Pr", "Prv"],
    },
    // 21. Eclesiastes
    BookMeta {
        osis: "Eccl",
        name_pt: "Eclesiastes",
        name_en: "Ecclesiastes",
        abbr_pt: &["Ec", "Ecl"],
        abbr_en: &["Eccl", "Ecc", "Ec"],
    },
    // 22. Cânticos
    BookMeta {
        osis: "Song",
        name_pt: "Cânticos",
        name_en: "Song of Songs",
        abbr_pt: &["Ct", "Cant", "Cantares", "Cânticos"],
        abbr_en: &["Song", "So", "Canticles", "Sos"],
    },
    // 23. Isaías
    BookMeta {
        osis: "Isa",
        name_pt: "Isaías",
        name_en: "Isaiah",
        abbr_pt: &["Is", "Isa"],
        abbr_en: &["Isa", "Is"],
    },
    // 24. Jeremias
    BookMeta {
        osis: "Jer",
        name_pt: "Jeremias",
        name_en: "Jeremiah",
        abbr_pt: &["Jr", "Jer"],
        abbr_en: &["Jer", "Je", "Jr"],
    },
    // 25. Lamentações
    BookMeta {
        osis: "Lam",
        name_pt: "Lamentações",
        name_en: "Lamentations",
        abbr_pt: &["Lm", "Lam"],
        abbr_en: &["Lam", "La"],
    },
    // 26. Ezequiel
    BookMeta {
        osis: "Ezek",
        name_pt: "Ezequiel",
        name_en: "Ezekiel",
        abbr_pt: &["Ez", "Eze", "Ezeq"],
        abbr_en: &["Ezek", "Eze", "Ezk"],
    },
    // 27. Daniel
    BookMeta {
        osis: "Dan",
        name_pt: "Daniel",
        name_en: "Daniel",
        abbr_pt: &["Dn", "Dan"],
        abbr_en: &["Dan", "Da", "Dn"],
    },
    // 28. Oseias
    BookMeta {
        osis: "Hos",
        name_pt: "Oseias",
        name_en: "Hosea",
        abbr_pt: &["Os", "Ose"],
        abbr_en: &["Hos", "Ho"],
    },
    // 29. Joel
    BookMeta {
        osis: "Joel",
        name_pt: "Joel",
        name_en: "Joel",
        abbr_pt: &["Jl", "Joe", "Joel"],
        abbr_en: &["Joel", "Joe", "Jl"],
    },
    // 30. Amós
    BookMeta {
        osis: "Amos",
        name_pt: "Amós",
        name_en: "Amos",
        abbr_pt: &["Am", "Amo"],
        abbr_en: &["Amos", "Am"],
    },
    // 31. Obadias
    BookMeta {
        osis: "Obad",
        name_pt: "Obadias",
        name_en: "Obadiah",
        abbr_pt: &["Ob", "Oba"],
        abbr_en: &["Obad", "Ob"],
    },
    // 32. Jonas
    BookMeta {
        osis: "Jonah",
        name_pt: "Jonas",
        name_en: "Jonah",
        abbr_pt: &["Jn", "Jon"],
        abbr_en: &["Jonah", "Jon", "Jnh"],
    },
    // 33. Miqueias
    BookMeta {
        osis: "Mic",
        name_pt: "Miqueias",
        name_en: "Micah",
        abbr_pt: &["Mq", "Miq"],
        abbr_en: &["Mic", "Mc"],
    },
    // 34. Naum
    BookMeta {
        osis: "Nah",
        name_pt: "Naum",
        name_en: "Nahum",
        abbr_pt: &["Na", "Nau"],
        abbr_en: &["Nah", "Na"],
    },
    // 35. Habacuque
    BookMeta {
        osis: "Hab",
        name_pt: "Habacuque",
        name_en: "Habakkuk",
        abbr_pt: &["Hc", "Hab"],
        abbr_en: &["Hab", "Hb"],
    },
    // 36. Sofonias
    BookMeta {
        osis: "Zeph",
        name_pt: "Sofonias",
        name_en: "Zephaniah",
        abbr_pt: &["Sf", "Sof"],
        abbr_en: &["Zeph", "Zep", "Zp"],
    },
    // 37. Ageu
    BookMeta {
        osis: "Hag",
        name_pt: "Ageu",
        name_en: "Haggai",
        abbr_pt: &["Ag", "Age"],
        abbr_en: &["Hag", "Hg"],
    },
    // 38. Zacarias
    BookMeta {
        osis: "Zech",
        name_pt: "Zacarias",
        name_en: "Zechariah",
        abbr_pt: &["Zc", "Zac"],
        abbr_en: &["Zech", "Zec", "Zc"],
    },
    // 39. Malaquias
    BookMeta {
        osis: "Mal",
        name_pt: "Malaquias",
        name_en: "Malachi",
        abbr_pt: &["Ml", "Mal"],
        abbr_en: &["Mal", "Ml"],
    },
    // 40. Mateus
    BookMeta {
        osis: "Matt",
        name_pt: "Mateus",
        name_en: "Matthew",
        abbr_pt: &["Mt", "Mat"],
        abbr_en: &["Matt", "Mat", "Mt"],
    },
    // 41. Marcos
    BookMeta {
        osis: "Mark",
        name_pt: "Marcos",
        name_en: "Mark",
        abbr_pt: &["Mc", "Mar"],
        abbr_en: &["Mark", "Mrk", "Mk", "Mr"],
    },
    // 42. Lucas
    BookMeta {
        osis: "Luke",
        name_pt: "Lucas",
        name_en: "Luke",
        abbr_pt: &["Lc", "Luc"],
        abbr_en: &["Luke", "Luk", "Lk"],
    },
    // 43. João
    BookMeta {
        osis: "John",
        name_pt: "João",
        name_en: "John",
        abbr_pt: &["Jo", "João", "Joa"],
        abbr_en: &["John", "Joh", "Jn", "Jhn"],
    },
    // 44. Atos
    BookMeta {
        osis: "Acts",
        name_pt: "Atos",
        name_en: "Acts",
        abbr_pt: &["At", "Ato", "Atos"],
        abbr_en: &["Acts", "Act", "Ac"],
    },
    // 45. Romanos
    BookMeta {
        osis: "Rom",
        name_pt: "Romanos",
        name_en: "Romans",
        abbr_pt: &["Rm", "Rom"],
        abbr_en: &["Rom", "Ro", "Rm"],
    },
    // 46. 1 Coríntios
    BookMeta {
        osis: "1Cor",
        name_pt: "1 Coríntios",
        name_en: "1 Corinthians",
        abbr_pt: &["1Co", "1 Co", "1Cor", "1 Cor"],
        abbr_en: &["1Cor", "1 Cor", "1Co", "1 Co"],
    },
    // 47. 2 Coríntios
    BookMeta {
        osis: "2Cor",
        name_pt: "2 Coríntios",
        name_en: "2 Corinthians",
        abbr_pt: &["2Co", "2 Co", "2Cor", "2 Cor"],
        abbr_en: &["2Cor", "2 Cor", "2Co", "2 Co"],
    },
    // 48. Gálatas
    BookMeta {
        osis: "Gal",
        name_pt: "Gálatas",
        name_en: "Galatians",
        abbr_pt: &["Gl", "Gal"],
        abbr_en: &["Gal", "Ga"],
    },
    // 49. Efésios
    BookMeta {
        osis: "Eph",
        name_pt: "Efésios",
        name_en: "Ephesians",
        abbr_pt: &["Ef", "Efe"],
        abbr_en: &["Eph", "Ep"],
    },
    // 50. Filipenses
    BookMeta {
        osis: "Phil",
        name_pt: "Filipenses",
        name_en: "Philippians",
        abbr_pt: &["Fp", "Fil"],
        abbr_en: &["Phil", "Php", "Pp"],
    },
    // 51. Colossenses
    BookMeta {
        osis: "Col",
        name_pt: "Colossenses",
        name_en: "Colossians",
        abbr_pt: &["Cl", "Col"],
        abbr_en: &["Col", "Co"],
    },
    // 52. 1 Tessalonicenses
    BookMeta {
        osis: "1Thess",
        name_pt: "1 Tessalonicenses",
        name_en: "1 Thessalonians",
        abbr_pt: &["1Ts", "1 Ts", "1Tes", "1 Tes"],
        abbr_en: &["1Thess", "1 Thess", "1Th", "1 Th"],
    },
    // 53. 2 Tessalonicenses
    BookMeta {
        osis: "2Thess",
        name_pt: "2 Tessalonicenses",
        name_en: "2 Thessalonians",
        abbr_pt: &["2Ts", "2 Ts", "2Tes", "2 Tes"],
        abbr_en: &["2Thess", "2 Thess", "2Th", "2 Th"],
    },
    // 54. 1 Timóteo
    BookMeta {
        osis: "1Tim",
        name_pt: "1 Timóteo",
        name_en: "1 Timothy",
        abbr_pt: &["1Tm", "1 Tm", "1Tim", "1 Tim"],
        abbr_en: &["1Tim", "1 Tim", "1Ti", "1 Ti"],
    },
    // 55. 2 Timóteo
    BookMeta {
        osis: "2Tim",
        name_pt: "2 Timóteo",
        name_en: "2 Timothy",
        abbr_pt: &["2Tm", "2 Tm", "2Tim", "2 Tim"],
        abbr_en: &["2Tim", "2 Tim", "2Ti", "2 Ti"],
    },
    // 56. Tito
    BookMeta {
        osis: "Titus",
        name_pt: "Tito",
        name_en: "Titus",
        abbr_pt: &["Tt", "Tit"],
        abbr_en: &["Titus", "Tit", "Ti"],
    },
    // 57. Filemom
    BookMeta {
        osis: "Phlm",
        name_pt: "Filemom",
        name_en: "Philemon",
        abbr_pt: &["Fm", "Flm"],
        abbr_en: &["Phlm", "Phm", "Pm"],
    },
    // 58. Hebreus
    BookMeta {
        osis: "Heb",
        name_pt: "Hebreus",
        name_en: "Hebrews",
        abbr_pt: &["Hb", "Heb"],
        abbr_en: &["Heb", "He"],
    },
    // 59. Tiago
    BookMeta {
        osis: "Jas",
        name_pt: "Tiago",
        name_en: "James",
        abbr_pt: &["Tg", "Tia"],
        abbr_en: &["Jas", "Ja", "Jam"],
    },
    // 60. 1 Pedro
    BookMeta {
        osis: "1Pet",
        name_pt: "1 Pedro",
        name_en: "1 Peter",
        abbr_pt: &["1Pe", "1 Pe", "1Ped", "1 Ped"],
        abbr_en: &["1Pet", "1 Pet", "1Pe", "1 Pe"],
    },
    // 61. 2 Pedro
    BookMeta {
        osis: "2Pet",
        name_pt: "2 Pedro",
        name_en: "2 Peter",
        abbr_pt: &["2Pe", "2 Pe", "2Ped", "2 Ped"],
        abbr_en: &["2Pet", "2 Pet", "2Pe", "2 Pe"],
    },
    // 62. 1 João
    BookMeta {
        osis: "1John",
        name_pt: "1 João",
        name_en: "1 John",
        abbr_pt: &["1Jo", "1 Jo", "1Joa", "1 Joa"],
        abbr_en: &["1John", "1 John", "1Jn", "1 Jn"],
    },
    // 63. 2 João
    BookMeta {
        osis: "2John",
        name_pt: "2 João",
        name_en: "2 John",
        abbr_pt: &["2Jo", "2 Jo", "2Joa", "2 Joa"],
        abbr_en: &["2John", "2 John", "2Jn", "2 Jn"],
    },
    // 64. 3 João
    BookMeta {
        osis: "3John",
        name_pt: "3 João",
        name_en: "3 John",
        abbr_pt: &["3Jo", "3 Jo", "3Joa", "3 Joa"],
        abbr_en: &["3John", "3 John", "3Jn", "3 Jn"],
    },
    // 65. Judas
    BookMeta {
        osis: "Jude",
        name_pt: "Judas",
        name_en: "Jude",
        abbr_pt: &["Jd", "Jud"],
        abbr_en: &["Jude", "Jud", "Jd"],
    },
    // 66. Apocalipse
    BookMeta {
        osis: "Rev",
        name_pt: "Apocalipse",
        name_en: "Revelation",
        abbr_pt: &["Ap", "Apoc"],
        abbr_en: &["Rev", "Re", "Rv"],
    },
];

pub const CANONICAL_CHAPTER_VERSES: [&[u16]; 66] = [
    // 1. Genesis (50)
    &[
        31, 25, 24, 26, 32, 22, 24, 22, 29, 32, 32, 20, 18, 24, 21, 16, 27, 33, 38, 18, 34, 24, 20,
        67, 34, 35, 46, 22, 35, 43, 55, 32, 20, 31, 29, 43, 36, 30, 23, 23, 57, 38, 34, 34, 28, 34,
        31, 22, 33, 26,
    ],
    // 2. Exodus (40)
    &[
        22, 25, 22, 31, 23, 30, 25, 32, 35, 29, 10, 51, 22, 31, 27, 36, 16, 27, 25, 26, 36, 31, 33,
        18, 40, 37, 21, 43, 46, 38, 18, 35, 23, 35, 35, 38, 29, 31, 43, 38,
    ],
    // 3. Leviticus (27)
    &[
        17, 16, 17, 35, 19, 30, 38, 36, 24, 20, 47, 8, 59, 57, 33, 34, 16, 30, 37, 27, 24, 33, 44,
        23, 55, 46, 34,
    ],
    // 4. Numbers (36)
    &[
        54, 34, 51, 49, 31, 27, 89, 26, 23, 36, 35, 16, 33, 45, 41, 50, 13, 32, 22, 29, 35, 41, 30,
        25, 18, 65, 23, 31, 40, 16, 54, 42, 56, 29, 34, 13,
    ],
    // 5. Deuteronomy (34)
    &[
        46, 37, 29, 49, 33, 25, 26, 20, 29, 22, 32, 32, 18, 29, 23, 22, 20, 22, 21, 20, 23, 30, 25,
        22, 19, 19, 26, 68, 29, 20, 30, 52, 29, 12,
    ],
    // 6. Joshua (24)
    &[
        18, 24, 17, 24, 15, 27, 26, 35, 27, 43, 23, 24, 33, 15, 63, 10, 18, 28, 51, 9, 45, 34, 16,
        33,
    ],
    // 7. Judges (21)
    &[
        36, 23, 31, 24, 31, 40, 25, 35, 57, 18, 40, 15, 25, 20, 20, 31, 13, 31, 30, 48, 25,
    ],
    // 8. Ruth (4)
    &[22, 23, 18, 22],
    // 9. 1 Samuel (31)
    &[
        28, 36, 21, 22, 12, 21, 17, 22, 27, 27, 15, 25, 23, 52, 35, 23, 58, 30, 24, 42, 15, 23, 29,
        22, 44, 25, 12, 25, 11, 31, 13,
    ],
    // 10. 2 Samuel (24)
    &[
        27, 32, 39, 12, 25, 23, 29, 18, 13, 19, 27, 31, 39, 33, 37, 23, 29, 33, 43, 26, 22, 51, 39,
        25,
    ],
    // 11. 1 Kings (22)
    &[
        53, 46, 28, 34, 18, 38, 51, 66, 28, 29, 43, 33, 34, 31, 34, 34, 24, 46, 21, 43, 29, 53,
    ],
    // 12. 2 Kings (25)
    &[
        18, 25, 27, 44, 27, 33, 20, 29, 37, 36, 21, 21, 25, 29, 38, 20, 41, 37, 37, 21, 26, 20, 37,
        20, 30,
    ],
    // 13. 1 Chronicles (29)
    &[
        54, 55, 24, 43, 26, 81, 40, 40, 44, 14, 47, 40, 14, 17, 29, 43, 27, 17, 19, 8, 30, 19, 32,
        31, 31, 32, 34, 21, 30,
    ],
    // 14. 2 Chronicles (36)
    &[
        17, 18, 17, 22, 14, 42, 22, 18, 31, 19, 23, 16, 22, 15, 19, 14, 19, 34, 11, 37, 20, 12, 21,
        27, 28, 23, 9, 27, 36, 27, 21, 33, 25, 33, 27, 23,
    ],
    // 15. Ezra (10)
    &[11, 70, 13, 24, 17, 22, 28, 36, 15, 44],
    // 16. Nehemiah (13)
    &[11, 20, 32, 23, 19, 19, 73, 18, 38, 39, 36, 47, 31],
    // 17. Esther (10)
    &[22, 23, 15, 17, 14, 14, 10, 17, 32, 3],
    // 18. Job (42)
    &[
        22, 13, 26, 21, 27, 30, 21, 22, 35, 22, 20, 25, 28, 22, 35, 22, 16, 21, 29, 29, 34, 30, 17,
        25, 6, 14, 23, 28, 25, 31, 40, 22, 33, 37, 16, 33, 24, 41, 30, 24, 34, 17,
    ],
    // 19. Psalms (150)
    &[
        6, 12, 8, 8, 12, 10, 17, 9, 20, 18, 7, 8, 6, 7, 5, 11, 15, 50, 14, 9, 13, 31, 6, 10, 22,
        12, 14, 9, 11, 12, 24, 11, 22, 22, 28, 12, 40, 22, 13, 17, 13, 11, 5, 26, 17, 11, 9, 14,
        20, 23, 19, 9, 6, 7, 23, 13, 11, 11, 17, 12, 8, 12, 11, 10, 13, 20, 7, 35, 36, 5, 24, 20,
        28, 23, 10, 12, 20, 72, 13, 19, 16, 8, 18, 12, 13, 17, 7, 18, 52, 17, 16, 15, 5, 23, 11,
        13, 12, 9, 9, 5, 8, 28, 22, 35, 45, 48, 43, 13, 31, 7, 10, 10, 9, 8, 18, 19, 2, 29, 176, 7,
        8, 9, 4, 8, 5, 6, 5, 6, 8, 8, 3, 18, 3, 3, 21, 26, 9, 8, 24, 13, 10, 7, 12, 15, 21, 10, 20,
        14, 9, 6,
    ],
    // 20. Proverbs (31)
    &[
        33, 22, 35, 27, 23, 35, 27, 36, 18, 32, 31, 28, 25, 35, 33, 33, 28, 24, 29, 30, 31, 29, 35,
        34, 28, 28, 27, 28, 27, 33, 31,
    ],
    // 21. Ecclesiastes (12)
    &[18, 26, 22, 16, 20, 12, 29, 17, 18, 20, 10, 14],
    // 22. Song of Songs (8)
    &[17, 17, 11, 16, 16, 13, 13, 14],
    // 23. Isaiah (66)
    &[
        31, 22, 26, 6, 30, 13, 25, 22, 21, 34, 16, 6, 22, 32, 9, 14, 14, 7, 25, 6, 17, 25, 18, 23,
        12, 21, 13, 29, 24, 33, 9, 20, 24, 17, 10, 22, 38, 22, 8, 31, 29, 25, 28, 28, 25, 13, 15,
        22, 26, 11, 23, 15, 12, 17, 13, 12, 21, 14, 21, 22, 11, 12, 19, 12, 25, 24,
    ],
    // 24. Jeremiah (52)
    &[
        19, 37, 25, 31, 31, 30, 34, 22, 26, 25, 23, 17, 27, 22, 21, 21, 27, 23, 15, 18, 14, 30, 40,
        10, 38, 24, 22, 17, 32, 24, 40, 44, 26, 22, 19, 32, 21, 28, 18, 16, 18, 22, 13, 30, 5, 28,
        7, 47, 39, 46, 64, 34,
    ],
    // 25. Lamentations (5)
    &[22, 22, 66, 22, 22],
    // 26. Ezekiel (48)
    &[
        28, 10, 27, 17, 17, 14, 27, 18, 11, 22, 25, 28, 23, 23, 8, 63, 24, 32, 14, 49, 32, 31, 49,
        27, 17, 21, 36, 26, 21, 26, 18, 32, 33, 31, 15, 38, 28, 23, 29, 49, 26, 20, 27, 31, 25, 24,
        23, 35,
    ],
    // 27. Daniel (12)
    &[21, 49, 30, 37, 31, 28, 28, 27, 27, 21, 45, 13],
    // 28. Hosea (14)
    &[11, 23, 5, 19, 15, 11, 16, 14, 17, 15, 12, 14, 16, 9],
    // 29. Joel (3)
    &[20, 32, 21],
    // 30. Amos (9)
    &[15, 16, 15, 13, 27, 14, 17, 14, 15],
    // 31. Obadiah (1)
    &[21],
    // 32. Jonah (4)
    &[17, 10, 10, 11],
    // 33. Micah (7)
    &[16, 13, 12, 13, 15, 16, 20],
    // 34. Nahum (3)
    &[15, 13, 19],
    // 35. Habakkuk (3)
    &[17, 20, 19],
    // 36. Zephaniah (3)
    &[18, 15, 20],
    // 37. Haggai (2)
    &[15, 23],
    // 38. Zechariah (14)
    &[21, 13, 10, 14, 11, 15, 14, 23, 17, 12, 17, 14, 9, 21],
    // 39. Malachi (4)
    &[14, 17, 18, 6],
    // 40. Matthew (28)
    &[
        25, 23, 17, 25, 48, 34, 29, 34, 38, 42, 30, 50, 58, 36, 39, 28, 27, 35, 30, 34, 46, 46, 39,
        51, 46, 75, 66, 20,
    ],
    // 41. Mark (16)
    &[
        45, 28, 35, 41, 43, 56, 37, 38, 50, 52, 33, 44, 37, 72, 47, 20,
    ],
    // 42. Luke (24)
    &[
        80, 52, 38, 44, 39, 49, 50, 56, 62, 42, 54, 59, 35, 35, 32, 31, 37, 43, 48, 47, 38, 71, 56,
        53,
    ],
    // 43. John (21)
    &[
        51, 25, 36, 54, 47, 71, 53, 59, 41, 42, 57, 50, 38, 31, 27, 33, 26, 40, 42, 31, 25,
    ],
    // 44. Acts (28)
    &[
        26, 47, 26, 37, 42, 15, 60, 40, 43, 48, 30, 25, 52, 28, 41, 40, 34, 28, 41, 38, 40, 30, 35,
        27, 27, 32, 44, 31,
    ],
    // 45. Romans (16)
    &[
        32, 29, 31, 25, 21, 23, 25, 39, 33, 21, 36, 21, 14, 23, 33, 27,
    ],
    // 46. 1 Corinthians (16)
    &[
        31, 16, 23, 21, 13, 20, 40, 13, 27, 33, 34, 31, 13, 40, 58, 24,
    ],
    // 47. 2 Corinthians (13)
    &[24, 17, 18, 18, 21, 18, 16, 24, 15, 18, 33, 21, 14],
    // 48. Galatians (6)
    &[24, 21, 29, 31, 26, 18],
    // 49. Ephesians (6)
    &[23, 22, 21, 32, 33, 24],
    // 50. Philippians (4)
    &[30, 30, 21, 23],
    // 51. Colossians (4)
    &[29, 23, 25, 18],
    // 52. 1 Thessalonians (5)
    &[10, 20, 13, 18, 28],
    // 53. 2 Thessalonians (3)
    &[12, 17, 18],
    // 54. 1 Timothy (6)
    &[20, 15, 16, 16, 25, 21],
    // 55. 2 Timothy (4)
    &[18, 26, 17, 22],
    // 56. Titus (3)
    &[16, 15, 15],
    // 57. Philemon (1)
    &[25],
    // 58. Hebrews (13)
    &[14, 18, 19, 16, 14, 20, 28, 13, 28, 39, 40, 29, 25],
    // 59. James (5)
    &[27, 26, 18, 17, 20],
    // 60. 1 Peter (5)
    &[25, 25, 22, 19, 14],
    // 61. 2 Peter (3)
    &[21, 22, 18],
    // 62. 1 John (5)
    &[10, 29, 24, 21, 21],
    // 63. 2 John (1)
    &[13],
    // 64. 3 John (1)
    &[14],
    // 65. Jude (1)
    &[25],
    // 66. Revelation (22)
    &[
        20, 29, 22, 11, 14, 17, 17, 13, 21, 11, 19, 17, 18, 20, 8, 21, 18, 24, 21, 15, 27, 21,
    ],
];

#[inline]
pub const fn book_meta(book: Book) -> &'static BookMeta {
    &BOOK_METADATA[(book.number() - 1) as usize]
}

#[inline]
pub fn chapter_count(book: Book) -> u16 {
    CANONICAL_CHAPTER_VERSES[(book.number() - 1) as usize].len() as u16
}

pub fn verses_in_chapter(book: Book, chapter: u16) -> Option<u16> {
    if chapter == 0 {
        return None;
    }
    let chapters = CANONICAL_CHAPTER_VERSES[(book.number() - 1) as usize];
    chapters.get((chapter - 1) as usize).copied()
}

pub fn total_verses_in_book(book: Book) -> u16 {
    let chapters = CANONICAL_CHAPTER_VERSES[(book.number() - 1) as usize];
    chapters.iter().sum()
}

pub fn cumulative_verse_index(verse: &VerseRef) -> Option<usize> {
    verse.validate().ok()?;
    let mut total = 0usize;
    for &b in &Book::ALL {
        if b == verse.book {
            for ch in 1..verse.chapter {
                total += verses_in_chapter(b, ch).unwrap_or(0) as usize;
            }
            total += (verse.verse - 1) as usize;
            return Some(total);
        } else {
            total += total_verses_in_book(b) as usize;
        }
    }
    None
}

pub fn verse_from_cumulative_index(mut index: usize) -> Option<VerseRef> {
    if index >= TOTAL_CANONICAL_VERSES {
        return None;
    }
    for &b in &Book::ALL {
        let book_verses = total_verses_in_book(b) as usize;
        if index < book_verses {
            let ch_count = chapter_count(b);
            for ch in 1..=ch_count {
                let v_count = verses_in_chapter(b, ch).unwrap_or(0) as usize;
                if index < v_count {
                    return Some(VerseRef::new(b, ch, (index + 1) as u16));
                }
                index -= v_count;
            }
            return None;
        }
        index -= book_verses;
    }
    None
}
